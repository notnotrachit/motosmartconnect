import { fileURLToPath, URL } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;
const here = (path: string) => fileURLToPath(new URL(path, import.meta.url));

// Two pages: the main window and the per-app stream window that Rust opens as stream.html.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: {
    alias: { "@": here("./src") },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/ui-backup/**"] },
  },
  build: {
    target: "safari16",
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      input: {
        main: here("./index.html"),
        stream: here("./stream.html"),
      },
    },
  },
});
