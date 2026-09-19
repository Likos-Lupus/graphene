use super::{collection, operation::DiagnosticAnalysisOperation};
use crate::{
    context::ServiceContext, instance_service::repository::InstanceRepository, operation_lifecycle,
};
use graphene_content::ContentInventoryFingerprint;
use graphene_core::{
    DiagnosticCode, DiagnosticParameters, ErrorCode, ErrorKind, InstanceId, OperationController,
    Result,
};
use graphene_diagnostics::{
    ContentEnvironment, DiagnosticCompleteness, DiagnosticMode, DiagnosticRecommendation,
    DiagnosticReport, DiagnosticRequest, DiagnosticSourceSummary, DiagnosticVerificationPolicy,
    EvidenceSourceKind, JavaEvidence, JavaRequirementSnapshot, JavaRuntimeSnapshot,
    MAX_SOURCES_PER_REPORT, ParseLimits, ParsedFinding, RecommendationActionKind, RedactionContext,
    Redactor, assemble_findings, cause_undetermined_finding, correlate_content, correlate_java,
    correlate_verification, derive_recommendations, enrich_with_java_evidence, has_severe_finding,
    parse_sources, process_exit_finding,
};
use graphene_instance::{
    InstalledComponentKind, InstanceLockfile, InstanceStateFingerprint, VerificationMode,
};
use graphene_java::{JavaArchitecture, JavaVendor, compatibility_diagnostic};
use graphene_storage::MAX_LOCKFILE_BYTES;
use std::sync::Arc;

pub(crate) fn start_analysis(
    context: Arc<ServiceContext>,
    instance_id: InstanceId,
    request: DiagnosticRequest,
) -> DiagnosticAnalysisOperation {
    let controller = context.operations.create("diagnostic-analyze");
    let operation = controller.handle();
    let future = Box::pin(async move {
        operation_lifecycle::start(&controller)?;
        let result = analyze_inner(&context, instance_id, request, &controller).await;
        operation_lifecycle::finish(
            &controller,
            result,
            ErrorCode::OperationCancelled,
            ErrorKind::Cancelled,
            "diagnostic analysis operation was cancelled",
        )
    });
    DiagnosticAnalysisOperation::new(operation, future)
}

async fn analyze_inner(
    context: &Arc<ServiceContext>,
    instance_id: InstanceId,
    request: DiagnosticRequest,
    controller: &OperationController,
) -> Result<DiagnosticReport> {
    request.validate()?;
    let repo = InstanceRepository::new(context.storage.path());

    controller.set_stage("snapshot")?;
    let _snapshot_lease = repo.acquire_shared_lease(instance_id)?;

    let verify_mode = match request.verification {
        DiagnosticVerificationPolicy::Quick => Some(VerificationMode::Quick),
        DiagnosticVerificationPolicy::Full => Some(VerificationMode::Full),
        DiagnosticVerificationPolicy::Skip => None,
    };
    let verification_report = match verify_mode {
        Some(mode) => {
            controller.set_stage("verify")?;
            Some(
                crate::instance_service::verification::execute_verify(
                    Arc::clone(context),
                    instance_id,
                    mode,
                    controller,
                )
                .await?,
            )
        }
        None => None,
    };

    let base_state = match &verification_report {
        Some(report) => report.state_fingerprint,
        None => compute_state_fingerprint(&repo, instance_id)?,
    };

    let java_evidence = if request.sources.collect_java {
        controller.set_stage("java")?;
        collect_java_evidence(context, instance_id).await
    } else {
        JavaEvidence::default()
    };

    controller.set_stage("collect_logs")?;
    let collected = collection::collect_sources(
        repo.paths(),
        instance_id,
        &request.sources,
        &controller.cancellation_token(),
    )?;

    let mut content_inventory = None;
    let mut content_fingerprint: Option<ContentInventoryFingerprint> = None;

    if request.sources.scan_content {
        controller.set_stage("scan_content")?;
        let inventory = crate::content_service::inventory::scan_instance_inventory(
            context,
            instance_id,
            true,
            controller,
        )
        .await?;
        content_fingerprint = Some(inventory.fingerprint);
        content_inventory = Some(inventory);
    }

    let mut completeness = collected.completeness;
    if verification_report.is_some()
        && let Ok(after) = compute_state_fingerprint(&repo, instance_id)
        && after != base_state
    {
        completeness = DiagnosticCompleteness::Stale;
    }

    if let Some(before) = &content_fingerprint {
        let after = crate::content_service::inventory::scan_instance_inventory(
            context,
            instance_id,
            true,
            controller,
        )
        .await?;
        if &after.fingerprint != before {
            completeness = DiagnosticCompleteness::Stale;
        }
    }

    controller.set_stage("analyze")?;
    let environment = ContentEnvironment {
        loader: detect_loader(&repo, instance_id),
        server: false,
    };
    let redactor = build_redactor(context);

    let mut parsed: Vec<ParsedFinding> = Vec::new();
    if let Some(report) = &verification_report {
        parsed.extend(correlate_verification(report));
    }

    parsed.extend(correlate_java(&java_evidence));
    if let Some(inventory) = &content_inventory {
        parsed.extend(correlate_content(inventory, Some(&environment)));
    }

    parsed.extend(parse_sources(
        &collected.text_sources,
        &redactor,
        &ParseLimits::default(),
    ));
    if let Some(exit) = &request.process_exit
        && let Some(finding) = process_exit_finding(exit)
    {
        parsed.push(finding);
    }

    parsed = enrich_with_java_evidence(&parsed, &java_evidence);
    if request.mode == DiagnosticMode::Crash && !has_severe_finding(&parsed) {
        parsed.push(cause_undetermined_finding());
    }

    controller.set_stage("redact")?;
    let (findings, evidence, mut truncation) = assemble_findings(&parsed);
    let mut recommendations = derive_recommendations(&findings);
    append_extra_recommendations(&mut recommendations, &request, completeness);

    let mut sources = collected.summaries;
    if verification_report.is_some() {
        sources.push(DiagnosticSourceSummary::new(
            EvidenceSourceKind::VerificationReport,
            0,
            false,
        ));
    }

    if request.sources.collect_java {
        sources.push(DiagnosticSourceSummary::new(
            EvidenceSourceKind::JavaEvidence,
            0,
            false,
        ));
    }

    if let Some(inventory) = &content_inventory {
        sources.push(
            DiagnosticSourceSummary::new(EvidenceSourceKind::ContentInventory, 0, false)
                .with_note(format!("{} files", inventory.files.len())),
        );
    }

    if request.process_exit.is_some() {
        sources.push(DiagnosticSourceSummary::new(
            EvidenceSourceKind::ProcessExit,
            0,
            false,
        ));
    }

    if sources.len() > MAX_SOURCES_PER_REPORT {
        truncation.sources_omitted += sources.len() - MAX_SOURCES_PER_REPORT;
        sources.truncate(MAX_SOURCES_PER_REPORT);
    }

    let mut report = DiagnosticReport::new(instance_id, request.mode, base_state);
    report.content_fingerprint = content_fingerprint;
    report.completeness = completeness;
    report.findings = findings;
    report.evidence = evidence;
    report.recommendations = recommendations;
    report.sources = sources;
    report.truncation = truncation;

    controller.set_stage("complete")?;
    report.normalize();
    report.validate()?;
    Ok(report)
}

fn append_extra_recommendations(
    recommendations: &mut Vec<DiagnosticRecommendation>,
    request: &DiagnosticRequest,
    completeness: DiagnosticCompleteness,
) {
    if request.verification == DiagnosticVerificationPolicy::Skip
        && request.mode == DiagnosticMode::Full
        && !recommendations.iter().any(|recommendation| {
            recommendation.action == RecommendationActionKind::RunFullVerification
        })
    {
        recommendations.push(DiagnosticRecommendation::new(
            DiagnosticCode::new("RECOMMEND_RUN_FULL_VERIFICATION"),
            RecommendationActionKind::RunFullVerification,
            DiagnosticParameters::new(),
        ));
    }

    if completeness == DiagnosticCompleteness::Stale
        && !recommendations.iter().any(|recommendation| {
            recommendation.action == RecommendationActionKind::CollectAdditionalEvidence
        })
    {
        let mut parameters = DiagnosticParameters::new();
        parameters.insert("reason".to_owned(), "INSTANCE_STATE_CHANGED".to_owned());
        recommendations.push(DiagnosticRecommendation::new(
            DiagnosticCode::new("RECOMMEND_COLLECT_ADDITIONAL_EVIDENCE"),
            RecommendationActionKind::CollectAdditionalEvidence,
            parameters,
        ));
    }
}

async fn collect_java_evidence(
    context: &Arc<ServiceContext>,
    instance_id: InstanceId,
) -> JavaEvidence {
    let service = crate::java_service::JavaService::new(Arc::clone(context));
    let requirement = service.requirement_for_instance(instance_id).await.ok();
    let (selected, probe_failed) = match service.select_for_instance(instance_id, None).await {
        Ok(runtime) => (Some(runtime), false),
        Err(error) => (
            None,
            matches!(
                error.code,
                ErrorCode::JavaProbeFailed
                    | ErrorCode::JavaProbeTimeout
                    | ErrorCode::JavaRuntimeUnexecutable
            ),
        ),
    };
    let compatibility = requirement
        .as_ref()
        .map(|requirement| compatibility_diagnostic(requirement, selected.as_ref()));

    JavaEvidence {
        requirement: requirement
            .as_ref()
            .map(|requirement| JavaRequirementSnapshot {
                major_version: requirement.major_version,
                component_hint: requirement.component_hint.clone(),
            }),
        selected: selected.as_ref().map(|runtime| JavaRuntimeSnapshot {
            major_version: runtime.major_version,
            vendor: vendor_label(&runtime.vendor),
            architecture: architecture_label(runtime.architecture),
            version: runtime.version.clone(),
        }),
        compatibility,
        probe_failed,
    }
}

fn vendor_label(vendor: &JavaVendor) -> String {
    match vendor {
        JavaVendor::Adoptium => "adoptium",
        JavaVendor::Oracle => "oracle",
        JavaVendor::Microsoft => "microsoft",
        JavaVendor::Azul => "azul",
        JavaVendor::Amazon => "amazon",
        JavaVendor::GraalVm => "graalvm",
        _ => "other",
    }
    .to_owned()
}

fn architecture_label(architecture: JavaArchitecture) -> String {
    match architecture {
        JavaArchitecture::X86 => "x86",
        JavaArchitecture::X86_64 => "x86_64",
        JavaArchitecture::AArch64 => "aarch64",
        _ => "other",
    }
    .to_owned()
}

fn detect_loader(repo: &InstanceRepository, instance_id: InstanceId) -> Option<String> {
    let receipt = repo.load_receipt(instance_id).ok()?;

    for component in &receipt.components {
        if matches!(component.kind, InstalledComponentKind::Loader) {
            let uid = component.uid.to_lowercase();
            if uid.contains("fabric") {
                return Some("fabric".to_owned());
            }
            if uid.contains("neoforge") {
                return Some("neoforge".to_owned());
            }
            if uid.contains("forge") {
                return Some("forge".to_owned());
            }
        }
    }

    None
}

fn compute_state_fingerprint(
    repo: &InstanceRepository,
    instance_id: InstanceId,
) -> Result<InstanceStateFingerprint> {
    let receipt = repo.load_receipt(instance_id)?;
    let lockfile = load_lockfile(repo, instance_id);
    let config = repo.effective_config(instance_id).ok();

    Ok(InstanceStateFingerprint::compute(
        instance_id,
        &receipt,
        lockfile.as_ref(),
        config.as_ref(),
    ))
}

fn load_lockfile(repo: &InstanceRepository, instance_id: InstanceId) -> Option<InstanceLockfile> {
    let path = repo.paths().lockfile_path(instance_id);
    if !path.exists() {
        return None;
    }

    let bytes = graphene_storage::read_document_bounded(&path, MAX_LOCKFILE_BYTES).ok()?;
    let lockfile: InstanceLockfile = serde_json::from_slice(&bytes).ok()?;
    lockfile.validate().ok()?;
    Some(lockfile)
}

fn build_redactor(context: &Arc<ServiceContext>) -> Redactor {
    let mut redaction = RedactionContext::new();

    if let Some(root) = context.storage.path().to_str() {
        redaction = redaction.with_data_root(root);
    }

    if let Some(home) = home_directory() {
        redaction = redaction.with_home(home);
    }

    Redactor::new(&redaction)
}

fn home_directory() -> Option<String> {
    ["HOME", "USERPROFILE"]
        .iter()
        .find_map(std::env::var_os)
        .map(|value| value.to_string_lossy().into_owned())
}
