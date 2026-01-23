import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  base: "/devtools/",
  plugins: [react()],
  server: {
    hmr: {},
    proxy: {
      "/api": {
        target: "http://localhost:8920",
        changeOrigin: true,
      },
    },
  },
  define: {
    "process.env": {},
  },
});
