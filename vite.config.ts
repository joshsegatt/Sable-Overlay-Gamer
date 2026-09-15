import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Workspace target/ lives at repo root. Watching sable_lib.dll on
      // Windows throws EBUSY while the linker holds the file.
      ignored: ["**/src-tauri/**", "**/target/**", "**/binaries/**"],
    },
  },
}));
