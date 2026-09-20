import {describe, expect, it} from "vitest";
import {
    formatDiagnostic,
    formatEngine,
    formatError,
    formatGameEvent,
    formatInstance,
    formatOperation,
} from "../src/view";

describe("view formatting", () => {
    it("formats engine info", () => {
        expect(
            formatEngine({data_root: "/root", os: "Linux", architecture: "X86_64"}),
        ).toBe("Linux/X86_64 - /root");
    });

    it("falls back to the instance id when no display name exists", () => {
        expect(
            formatInstance({
                instance_id: "abc",
                display_name: null,
                status: "Ready",
                minecraft_version: null,
            }),
        ).toBe("abc [Ready] mc=?");
    });

    it("formats operation state, stage and terminal", () => {
        expect(
            formatOperation({
                id: "op",
                name: "verify",
                state: "Running",
                stage: "verify-files",
                terminal: null,
            }),
        ).toBe("verify Running (verify-files)");
        expect(
            formatOperation({
                id: "op",
                name: "verify",
                state: "Succeeded",
                stage: null,
                terminal: {state: "SUCCEEDED"},
            }),
        ).toBe("verify Succeeded -> SUCCEEDED");
    });

    it("formats a terminated game event", () => {
        expect(
            formatGameEvent({
                run_id: "run-1",
                kind: "exited",
                pid: null,
                text: null,
                success: true,
                exit_code: 0,
            }),
        ).toBe("exited: success=true code=0");
    });

    it("formats errors with context", () => {
        expect(
            formatError({
                code: "CONFIG_INVALID",
                kind: "Configuration",
                message: "invalid instance_id",
                context: {instance_id: "nope"},
            }),
        ).toBe("CONFIG_INVALID (Configuration): invalid instance_id [instance_id=nope]");
    });

    it("presents diagnostics by code, severity and confidence", () => {
        const lines = formatDiagnostic({
            mode: "Crash",
            completeness: "Complete",
            findings: [
                {
                    id: 1,
                    diagnostic: {code: "JVM_FATAL_ERROR", severity: "Critical"},
                    confidence: "Confirmed",
                },
            ],
            recommendations: [{code: "PLAN_REPAIR", action: "PlanInstanceRepair"}],
        });
        expect(lines[0]).toBe("mode=Crash completeness=Complete");
        expect(lines[1]).toContain("JVM_FATAL_ERROR");
        expect(lines[1]).toContain("Confirmed");
        expect(lines[2]).toContain("PlanInstanceRepair");
    });
});
