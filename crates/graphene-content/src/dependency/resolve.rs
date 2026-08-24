use crate::{
    compatibility::evaluate_compatibility,
    dependency::graph::{DependencyResolutionGraph, ResolvedDependencyClosure},
    model::{
        compatibility::{InstanceContentContext, ReleaseChannelPolicy},
        dependency::{DependencyRelation, DependencyTarget},
        version::ContentVersion,
    },
    provider::port::{ContentProvider, ContentVersionFilter},
};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, OperationController, Result};

pub const MAX_GRAPH_NODES: usize = 256;
pub const MAX_TRAVERSAL_DEPTH: usize = 32;
pub const MAX_PROVIDER_REQUESTS: usize = 64;

/// Resolves the complete required dependency closure for the selected root versions.
pub async fn resolve_dependencies<'a>(
    root_versions: &[ContentVersion],
    context: &InstanceContentContext,
    policy: ReleaseChannelPolicy,
    provider: &'a dyn ContentProvider,
    operation: &'a OperationController,
) -> Result<ResolvedDependencyClosure> {
    if root_versions.is_empty() {
        return Ok(ResolvedDependencyClosure {
            versions: Vec::new(),
            files: Vec::new(),
            optional_dependencies: Vec::new(),
        });
    }

    let mut graph = DependencyResolutionGraph::default();
    let mut queue: Vec<(ContentVersion, usize)> = Vec::new();

    // 1. Initialize root versions
    for root in root_versions {
        let compat = evaluate_compatibility(root, context, policy);
        if !compat.is_compatible() {
            return Err(GrapheneError::new(
                ErrorCode::ContentIncompatible,
                ErrorKind::Content,
                format!(
                    "root content version {} is incompatible with instance",
                    root.version_ref
                ),
            ));
        }

        let proj_ref = root.version_ref.project_ref();
        if let Some(existing) = graph.pinned_versions.get(&proj_ref) {
            if existing != &root.version_ref {
                return Err(GrapheneError::new(
                    ErrorCode::ContentDependencyConflict,
                    ErrorKind::Content,
                    format!(
                        "conflicting root versions requested for project {proj_ref}: {existing} vs {}",
                        root.version_ref
                    ),
                ));
            }
        } else {
            graph
                .pinned_versions
                .insert(proj_ref.clone(), root.version_ref.clone());
            graph
                .versions_by_ref
                .insert(root.version_ref.clone(), root.clone());
            queue.push((root.clone(), 0));
        }
    }

    // 2. Breadth-first traversal of required dependencies
    while let Some((current_version, depth)) = queue.pop() {
        if depth > MAX_TRAVERSAL_DEPTH {
            return Err(GrapheneError::new(
                ErrorCode::ContentDependencyBoundExceeded,
                ErrorKind::Content,
                format!("dependency traversal exceeded max depth of {MAX_TRAVERSAL_DEPTH}"),
            ));
        }

        if graph.pinned_versions.len() > MAX_GRAPH_NODES {
            return Err(GrapheneError::new(
                ErrorCode::ContentDependencyBoundExceeded,
                ErrorKind::Content,
                format!("dependency graph exceeded maximum node count of {MAX_GRAPH_NODES}"),
            ));
        }

        for dep in &current_version.dependencies {
            match dep.relation {
                DependencyRelation::Optional => {
                    graph.optional_dependencies.push(dep.clone());
                }
                DependencyRelation::Embedded | DependencyRelation::Unknown => {
                    // Embedded dependencies are already bundled inside the JAR
                }
                DependencyRelation::Incompatible => {
                    // Incompatible check
                    match &dep.target {
                        DependencyTarget::Project(p) => {
                            if graph.pinned_versions.contains_key(p) {
                                return Err(GrapheneError::new(
                                    ErrorCode::ContentDependencyConflict,
                                    ErrorKind::Content,
                                    format!(
                                        "content {} declared incompatibility with required project {p}",
                                        current_version.version_ref
                                    ),
                                ));
                            }
                        }
                        DependencyTarget::Version(v) => {
                            if graph.pinned_versions.values().any(|pinned| pinned == v) {
                                return Err(GrapheneError::new(
                                    ErrorCode::ContentDependencyConflict,
                                    ErrorKind::Content,
                                    format!(
                                        "content {} declared incompatibility with required version {v}",
                                        current_version.version_ref
                                    ),
                                ));
                            }
                        }
                        DependencyTarget::FilenameHint(_) => {}
                    }
                }
                DependencyRelation::Required => {
                    match &dep.target {
                        DependencyTarget::Version(target_vref) => {
                            let proj_ref = target_vref.project_ref();
                            if let Some(pinned) = graph.pinned_versions.get(&proj_ref) {
                                if pinned != target_vref {
                                    return Err(GrapheneError::new(
                                        ErrorCode::ContentDependencyConflict,
                                        ErrorKind::Content,
                                        format!(
                                            "dependency conflict: project {proj_ref} is pinned to {pinned} but {} requires {target_vref}",
                                            current_version.version_ref
                                        ),
                                    ));
                                }
                            } else {
                                graph.request_count += 1;
                                if graph.request_count > MAX_PROVIDER_REQUESTS {
                                    return Err(GrapheneError::new(
                                        ErrorCode::ContentDependencyBoundExceeded,
                                        ErrorKind::Content,
                                        format!(
                                            "dependency resolution exceeded max provider requests of {MAX_PROVIDER_REQUESTS}"
                                        ),
                                    ));
                                }

                                let fetched = provider.get_version(target_vref, operation).await?;
                                let compat = evaluate_compatibility(&fetched, context, policy);
                                if !compat.is_compatible() {
                                    return Err(GrapheneError::new(
                                        ErrorCode::ContentIncompatible,
                                        ErrorKind::Content,
                                        format!(
                                            "required dependency {target_vref} is incompatible with instance context"
                                        ),
                                    ));
                                }

                                graph.pinned_versions.insert(proj_ref, target_vref.clone());
                                graph
                                    .versions_by_ref
                                    .insert(target_vref.clone(), fetched.clone());
                                queue.push((fetched, depth + 1));
                            }
                        }
                        DependencyTarget::Project(target_pref) => {
                            if graph.pinned_versions.contains_key(target_pref) {
                                // Already pinned and satisfied
                                continue;
                            }

                            graph.request_count += 1;
                            if graph.request_count > MAX_PROVIDER_REQUESTS {
                                return Err(GrapheneError::new(
                                    ErrorCode::ContentDependencyBoundExceeded,
                                    ErrorKind::Content,
                                    format!(
                                        "dependency resolution exceeded max provider requests of {MAX_PROVIDER_REQUESTS}"
                                    ),
                                ));
                            }

                            let filter = ContentVersionFilter {
                                minecraft_version: Some(context.minecraft_version.clone()),
                                loader: context.loader,
                            };

                            let candidates = provider
                                .list_versions(target_pref, &filter, operation)
                                .await?;

                            // Filter for compatibility and verifiable file
                            let mut compatible_candidates: Vec<ContentVersion> = candidates
                                .into_iter()
                                .filter(|c| {
                                    evaluate_compatibility(c, context, policy).is_compatible()
                                        && c.primary_file().is_some()
                                })
                                .collect();

                            if compatible_candidates.is_empty() {
                                return Err(GrapheneError::new(
                                    ErrorCode::ContentDependencyUnsatisfied,
                                    ErrorKind::Content,
                                    format!(
                                        "no compatible version of required dependency project {target_pref} was found for Minecraft {}",
                                        context.minecraft_version
                                    ),
                                ));
                            }

                            // Sort by release channel, publication date, version string
                            compatible_candidates.sort_by(|a, b| {
                                a.release_channel
                                    .cmp(&b.release_channel)
                                    .then_with(|| b.date_published.cmp(&a.date_published))
                                    .then_with(|| b.version_number.cmp(&a.version_number))
                                    .then_with(|| {
                                        b.version_ref.version_id.cmp(&a.version_ref.version_id)
                                    })
                            });

                            let selected = compatible_candidates.remove(0);
                            let vref = selected.version_ref.clone();

                            graph
                                .pinned_versions
                                .insert(target_pref.clone(), vref.clone());
                            graph.versions_by_ref.insert(vref, selected.clone());
                            queue.push((selected, depth + 1));
                        }
                        DependencyTarget::FilenameHint(_) => {
                            // Filename hints cannot be queried from provider automatically
                        }
                    }
                }
            }
        }
    }

    let mut versions = Vec::new();
    let mut files = Vec::new();

    for vref in graph.pinned_versions.values() {
        if let Some(ver) = graph.versions_by_ref.get(vref) {
            let primary = ver.primary_file().ok_or_else(|| {
                GrapheneError::new(
                    ErrorCode::ContentFileUnverifiable,
                    ErrorKind::Content,
                    format!(
                        "version {vref} does not contain an unambiguous verifiable primary file"
                    ),
                )
            })?;

            versions.push(ver.clone());
            files.push(primary.clone());
        }
    }

    Ok(ResolvedDependencyClosure {
        versions,
        files,
        optional_dependencies: graph.optional_dependencies,
    })
}
