use crate::{content_service::inventory::scan_instance_inventory, context::ServiceContext};
use graphene_content::{ContentFileMatch, ExactMatchStatus, FileMatchItem, FileMatchRequest};
use graphene_core::{InstanceId, OperationController, Result};
use std::sync::Arc;

/// Explicitly recognizes local mods in an instance by matching hashes/fingerprints with remote providers.
pub async fn recognize_instance_files(
    context: &Arc<ServiceContext>,
    instance_id: InstanceId,
    operation: &OperationController,
) -> Result<Vec<ContentFileMatch>> {
    // 1. Scan local inventory computing hashes
    let inventory = scan_instance_inventory(context, instance_id, true, operation).await?;

    let items: Vec<FileMatchItem> = inventory
        .files
        .iter()
        .map(|f| FileMatchItem {
            sha1: f.sha1,
            sha256: f.sha256,
            murmur2_fingerprint: f.murmur2,
            size: f.size,
        })
        .collect();

    if items.is_empty() {
        return Ok(Vec::new());
    }

    let request = FileMatchRequest { items };
    let mut combined_matches = Vec::new();

    // Query each registered provider
    for (_, provider) in context.content_registry.iter() {
        if let Ok(matches) = provider.match_files(&request, operation).await {
            combined_matches.extend(matches);
        }
    }

    // Deduplicate/group matches by local item
    let mut final_results = Vec::new();
    for item in &request.items {
        let matched: Vec<_> = combined_matches
            .iter()
            .filter(|m| &m.item == item && matches!(m.status, ExactMatchStatus::Matched { .. }))
            .collect();

        if matched.len() == 1 {
            final_results.push(matched[0].clone());
        } else if matched.len() > 1 {
            let refs = matched
                .iter()
                .filter_map(|m| match &m.status {
                    ExactMatchStatus::Matched { file, .. } => Some(file.file_ref.clone()),
                    _ => None,
                })
                .collect();
            final_results.push(ContentFileMatch {
                item: item.clone(),
                status: ExactMatchStatus::Ambiguous(refs),
            });
        } else {
            final_results.push(ContentFileMatch {
                item: item.clone(),
                status: ExactMatchStatus::Unmatched,
            });
        }
    }

    Ok(final_results)
}
