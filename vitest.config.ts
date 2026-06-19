import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config";

export default mergeConfig(
  await viteConfig(),
  defineConfig({
    test: {
      globals: true,
      environment: "jsdom",
      setupFiles: ["./src/setupTests.ts"],
      // Phase 08.1 (Cert Split) — expand include to pick up tests
      // colocated with Studio-overlay components under `pro/src/`.
      include: [
        "src/**/*.{test,spec}.{ts,tsx}",
        "pro/src/**/*.{test,spec}.{ts,tsx}",
      ],
    },
  }),
);
