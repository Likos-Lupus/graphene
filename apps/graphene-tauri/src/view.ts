// Pure presentation helpers. They operate only on app-local wire shapes and never parse human
// diagnostic/error prose to discover codes.
import type {
    AccountSummary,
    AuthInteractionView,
    EngineInfo,
    GameEventWire,
    HostError,
    InstanceSummary,
    OperationSummary,
    RunSummaryView,
} from "./api";

export interface DiagnosticFinding {
    id: number;
    diagnostic: { code: string; severity: string };
    confidence: string;
}

export interface DiagnosticRecommendation {
    code: string;
    action: string;
}

export interface DiagnosticReport {
    mode: string;
    completeness: string;
    findings: DiagnosticFinding[];
    recommendations: DiagnosticRecommendation[];
}

export function formatEngine(info: EngineInfo): string {
    return `${info.os}/${info.architecture} - ${info.data_root}`;
}

export function formatInstance(instance: InstanceSummary): string {
    const name = instance.display_name ?? instance.instance_id;
    return `${name} [${instance.status}] mc=${instance.minecraft_version ?? "?"}`;
}

export function formatOperation(operation: OperationSummary): string {
    const stage = operation.stage
        ? ` (${operation.stage})`
        : "";
    const terminal = operation.terminal
        ? ` -> ${operation.terminal.state}`
        : "";
    return `${operation.name} ${operation.state}${stage}${terminal}`;
}

export function formatAccount(account: AccountSummary): string {
    return `${account.display_name} [${account.kind}/${account.state}]`;
}

export function formatAuthInteraction(view: AuthInteractionView): string {
    return `Open ${view.verification_uri} and enter ${view.user_code}`;
}

export function formatRun(run: RunSummaryView): string {
    return `${run.run_id} pid=${run.pid} dropped=${run.dropped_output_count}`;
}

export function formatGameEvent(event: GameEventWire): string {
    if (event.text !== null) {
        return `${event.kind}: ${event.text}`;
    }
    if (event.success !== null) {
        return `${event.kind}: success=${event.success} code=${event.exit_code ?? "none"}`;
    }
    return event.kind;
}

export function formatError(error: HostError): string {
    const context = Object.entries(error.context)
        .map(([key, value]) => `${key}=${value}`)
        .join(", ");
    return context
        ? `${error.code} (${error.kind}): ${error.message} [${context}]`
        : `${error.code} (${error.kind}): ${error.message}`;
}

export function formatDiagnostic(report: DiagnosticReport): string[] {
    const lines = [`mode=${report.mode} completeness=${report.completeness}`];
    for (const finding of report.findings) {
        lines.push(
            `  [${finding.id}] ${finding.diagnostic.code} ${finding.diagnostic.severity} ${finding.confidence}`,
        );
    }
    for (const recommendation of report.recommendations) {
        lines.push(`  -> ${recommendation.code} ${recommendation.action}`);
    }
    return lines;
}
