import { defineConfig } from "vite";

const isolation = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
  "Cross-Origin-Resource-Policy": "same-origin",
};

export default defineConfig({
  server: { host: "127.0.0.1", port: 5173, headers: isolation },
  preview: { host: "127.0.0.1", port: 4173, headers: isolation },
  build: {
    rollupOptions: {
      external: ["/vendor/mgba.js"],
    },
  },
});
