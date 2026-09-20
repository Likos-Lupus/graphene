import {beforeEach, describe, expect, it, vi} from "vitest";
import {commands, onOperationEvent, toHostError} from "../src/api";

const invokeMock = vi.fn();
const listenMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
    invoke: (...args: unknown[]) => invokeMock(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
    listen: (...args: unknown[]) => listenMock(...args),
}));

beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
});

describe("host api wrappers", () => {
    it("maps the engine info command", async () => {
        invokeMock.mockResolvedValue({
            data_root: "/root",
            os: "Linux",
            architecture: "X86_64",
        });
        const info = await commands.engineInfo();
        expect(invokeMock).toHaveBeenCalledWith("engine_info");
        expect(info.data_root).toBe("/root");
    });

    it("passes camelCase arguments for snake_case commands", async () => {
        invokeMock.mockResolvedValue({operation_id: "op"});
        await commands.startSynthetic(3, 10);
        expect(invokeMock).toHaveBeenCalledWith("engine_start_synthetic", {
            steps: 3,
            delayMs: 10,
        });
    });

    it("subscribes to the typed operation channel", async () => {
        listenMock.mockResolvedValue(() => {
        });
        const handler = vi.fn();
        await onOperationEvent(handler);
        expect(listenMock).toHaveBeenCalledWith("graphene://operation", expect.any(Function));
    });

    it("coerces unknown rejections into a stable envelope", () => {
        expect(toHostError(new Error("boom")).code).toBe("HOST_UNKNOWN");
        expect(
            toHostError({
                code: "CONFIG_INVALID",
                kind: "Configuration",
                message: "bad",
                context: {},
            }).code,
        ).toBe("CONFIG_INVALID");
    });
});
