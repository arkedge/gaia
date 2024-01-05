import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import pluginRewriteAll from "vite-plugin-rewrite-all";

console.log("crate root", process.env.DEVTOOLS_CRATE_ROOT);
export default defineConfig({
  resolve: {
    alias: [
      {
        find: "@crate/",
        // cargo build の際はbuild.rsによってDEVTOOLS_CRATE_ROOTが指定される
        replacement: (process.env.DEVTOOLS_CRATE_ROOT ?? "/crates") + "/",
      },
    ],
  },
  base: "/devtools/",
  plugins: [react(), pluginRewriteAll()],
  server: {
    hmr: {},
  },
  define: {
    "process.env": {},
  },
});
