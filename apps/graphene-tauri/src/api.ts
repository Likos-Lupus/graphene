// Typed host API wrappers. The frontend never sees engine internals, only these app-local wire
// shapes and the bounded event channels the Rust host emits.
import {invoke} from "@tauri-apps/api/core";
import {listen, type UnlistenFn} from "@tauri-apps/api/event";

export interface EngineInfo {
    data_root: string;
    os: string;
    architecture: string;
}

export interface InstanceSummary {
    instance_id: string;
    display_name: string | null;
    status: string;
    minecraft_version: string | null;
}

export interface OperationHandleView {
    operation_id: string;
}

export interface AccountSummary {
    account_id: string;
    kind: string;
    state: string;
    display_name: string;
}

export interface AuthInteractionView {
    operation_id: string;
    kind: string;
    verification_uri: string;
    user_code: string;
    expires_in_seconds: number;
    poll_interval_seconds: number;
}

export interface RunHandleView {
    run_id: string;
    pid: number;
}

export interface RunSummaryView {
    run_id: string;
    pid: number;
    dropped_output_count: number;
}

export interface OperationSummary {
    id: string;
    name: string;
    state: string;
    stage: string | null;
    terminal: { state: string } | null;
}

export interface OperationEventWire {
    operation_id: string;
    sequence: number;
    kind: Record<string, unknown>;
}

export interface GameEventWire {
    run_id: string;
    kind: string;
    pid: number | null;
    text: string | null;
    success: boolean | null;
    exit_code: number | null;
}

export interface HostError {
    code: string;
    kind: string;
    message: string;
    context: Record<string, string>;
}

export const OPERATION_EVENT = "graphene://operation";
export const OPERATION_TERMINAL_EVENT = "graphene://operation-terminal";
export const GAME_EVENT = "graphene://game";
export const AUTH_RESULT_EVENT = "graphene://auth-result";

export const commands = {
    engineInfo: () => invoke<EngineInfo>("engine_info"),
    startSynthetic: (steps: number, delayMs: number) =>
        invoke<OperationHandleView>("engine_start_synthetic", {steps, delayMs}),
    operationsList: () => invoke<OperationSummary[]>("operations_list"),
    cancelOperation: (operationId: string) =>
        invoke<void>("operations_cancel", {operationId}),
    instanceList: () => invoke<InstanceSummary[]>("instances_list"),
    instanceVerify: (instanceId: string, full: boolean) =>
        invoke<unknown>("instance_verify", {instanceId, full}),
    accountList: () => invoke<AccountSummary[]>("accounts_list"),
    accountOfflineAdd: (name: string) =>
        invoke<AccountSummary>("account_offline_add", {name}),
    beginMicrosoftLogin: () => invoke<AuthInteractionView>("account_begin_microsoft_login"),
    javaList: () => invoke<unknown[]>("java_list"),
    contentScan: (instanceId: string, hashes: boolean) =>
        invoke<unknown>("content_scan", {instanceId, hashes}),
    launchPlan: (instanceId: string, accountId: string) =>
        invoke<unknown>("launch_plan", {instanceId, accountId}),
    launchRun: (instanceId: string, accountId: string) =>
        invoke<RunHandleView>("launch_run", {instanceId, accountId}),
    launchKill: (runId: string) => invoke<void>("launch_kill", {runId}),
    runsList: () => invoke<RunSummaryView[]>("runs_list"),
    diagnose: (instanceId: string, mode: string, exitCode: number | null) =>
        invoke<unknown>("diagnostics_analyze", {instanceId, mode, exitCode}),
} as const;

export function onOperationEvent(
    handler: (event: OperationEventWire) => void,
): Promise<UnlistenFn> {
    return listen<OperationEventWire>(OPERATION_EVENT, (event) => handler(event.payload));
}

export function onOperationTerminal(
    handler: (event: { operation_id: string; terminal: { state: string } | null }) => void,
): Promise<UnlistenFn> {
    return listen(OPERATION_TERMINAL_EVENT, (event) =>
        handler(event.payload as { operation_id: string; terminal: { state: string } | null }),
    );
}

export function onGameEvent(handler: (event: GameEventWire) => void): Promise<UnlistenFn> {
    return listen<GameEventWire>(GAME_EVENT, (event) => handler(event.payload));
}

export function onAuthResult(handler: (event: unknown) => void): Promise<UnlistenFn> {
    return listen(AUTH_RESULT_EVENT, (event) => handler(event.payload));
}

/** Coerces an unknown rejection into an explicit, non-prose error envelope. */
export function toHostError(error: unknown): HostError {
    if (
        typeof error === "object" &&
        error !== null &&
        "code" in error &&
        "kind" in error &&
        "message" in error
    ) {
        return error as HostError;
    }
    return {
        code: "HOST_UNKNOWN",
        kind: "Internal",
        message: "unexpected host error",
        context: {},
    };
}
