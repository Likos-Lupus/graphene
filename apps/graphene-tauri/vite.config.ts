import {defineConfig} from "vitest/config";

// Tauri expects a fixed dev port and serves the built assets from `dist/` in production.
export default defineConfig({
    clearScreen: false,
    server: {
        port: 1420,
        strictPort: true,
    },
    build: {
        outDir: "dist",
        emptyOutDir: true,
        target: "es2022",
    },
    test: {
        environment: "node",
        include: ["tests/**/*.test.ts"],
    },
});
