import {commands, onGameEvent, onOperationEvent, onOperationTerminal, toHostError} from "./api";
import {
    formatAccount,
    formatEngine,
    formatError,
    formatGameEvent,
    formatInstance,
    formatOperation,
} from "./view";

// Minimal integration shell: it exercises typed commands and events only, never engine internals or
// human-message parsing. Full launcher design is intentionally out of scope.
const root = document.querySelector<HTMLElement>("#app");

if (root) {
    const output = document.createElement("pre");
    output.id = "output";

    const action = (label: string, run: () => Promise<string>): HTMLButtonElement => {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = label;
        button.addEventListener("click", async () => {
            try {
                output.textContent = await run();
            } catch (error) {
                output.textContent = formatError(toHostError(error));
            }
        });
        return button;
    };

    root.append(
        action("Engine status", async () => formatEngine(await commands.engineInfo())),
        action("Instances", async () =>
            (await commands.instanceList()).map(formatInstance).join("\n"),
        ),
        action("Accounts", async () =>
            (await commands.accountList()).map(formatAccount).join("\n"),
        ),
        action("Start operation", async () => {
            const handle = await commands.startSynthetic(5, 50);
            return `started ${handle.operation_id}`;
        }),
        action("Operations", async () =>
            (await commands.operationsList()).map(formatOperation).join("\n"),
        ),
        output,
    );

    void onOperationEvent((event) => {
        output.textContent += `\nop ${event.operation_id} #${event.sequence}`;
    });
    void onOperationTerminal((event) => {
        output.textContent += `\nop ${event.operation_id} terminal ${event.terminal?.state ?? "unknown"}`;
    });
    void onGameEvent((event) => {
        output.textContent += `\n${formatGameEvent(event)}`;
    });
}

export {};
