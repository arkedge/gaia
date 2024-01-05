import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import pluginRewriteAll from "vite-plugin-rewrite-all";

export default defineConfig({
  resolve: {
    alias: [
      {
        find: "@crate/",
        // wasmpackでrustソースから作られたpkgの場所を指定する
        // cargo build の際はbuild.rsによってDEVTOOLS_CRATE_ROOTが指定される
        // yarn dev している場合はrustソースのディレクトリに直接pkgを配置し、DEVTOOLS_CRATE_ROOTは指定されない
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
