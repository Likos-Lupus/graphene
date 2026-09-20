use crate::cli::{DiagnoseArgs, DiagnosticModeArg};
use crate::commands::{await_operation, parse_instance, progress_enabled};
use crate::context::AppContext;
use crate::output::Rendered;
use graphene::{DiagnosticReport, DiagnosticRequest, GrapheneError, ProcessExitEvidence};

pub async fn dispatch(context: &AppContext, args: DiagnoseArgs) -> Result<Rendered, GrapheneError> {
    let id = parse_instance(&args.instance_id)?;
    let request = build_request(args.mode, args.exit_code);
    let operation = context.engine.diagnostics().analyze(id, request);
    let handle = operation.operation();
    let report =
        await_operation(handle, operation.await_result(), progress_enabled(context)).await?;
    let human = report_human(&report);
    Ok(Rendered::new(human, Rendered::value(&report)))
}

fn build_request(mode: DiagnosticModeArg, exit_code: Option<i32>) -> DiagnosticRequest {
    let request = match mode {
        DiagnosticModeArg::Preflight => DiagnosticRequest::preflight(),
        DiagnosticModeArg::Crash => DiagnosticRequest::crash(),
        DiagnosticModeArg::Full => DiagnosticRequest::full(),
    };

    match exit_code {
        Some(code) => {
            request.with_process_exit(ProcessExitEvidence::new(Some(code), code == 0, false))
        }
        None => request,
    }
}

fn report_human(report: &DiagnosticReport) -> Vec<String> {
    let mut lines = vec![
        format!("mode: {:?}", report.mode),
        format!("completeness: {:?}", report.completeness),
        format!("findings: {}", report.findings.len()),
        format!("recommendations: {}", report.recommendations.len()),
    ];

    for finding in &report.findings {
        lines.push(format!(
            "  [{:?}] {} ({:?}, {:?})",
            finding.id,
            finding.diagnostic.code.as_str(),
            finding.diagnostic.severity,
            finding.confidence,
        ));
    }

    for recommendation in &report.recommendations {
        lines.push(format!(
            "  -> {} : {:?}",
            recommendation.code.as_str(),
            recommendation.action
        ));
    }

    lines
}
