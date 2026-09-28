import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "node",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    // @scout/query's dist imports its own files without extensions, which
    // Node's ESM loader rejects; let Vite resolve them (as the app build does).
    server: { deps: { inline: ["@scout/query"] } },
  },
});
