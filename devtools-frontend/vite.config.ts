import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  resolve: {},
  base: "/devtools/",
  plugins: [react()],
  server: {
    hmr: {},
  },
  define: {
    "process.env": {},
  },
});
