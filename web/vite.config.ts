/// <reference types="vitest" />
import fs from "node:fs";
import path from "node:path";
import { defineConfig, type Plugin } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";

function inlineSvgPlugin(): Plugin {
  return {
    name: "inline-svg-plugin",
    enforce: "pre",
    transformIndexHtml(html) {
      return html.replace(/<svg-icon\s+([^>]*?)(?:><\/svg-icon>|\/>)/gi, (_, attrStr) => {
        const srcMatch = attrStr.match(/src=["']([^"']+)["']/i);
        if (!srcMatch) return "";
        const src = srcMatch[1];
        const filePath = path.resolve(import.meta.dirname, src);

        if (!fs.existsSync(filePath)) {
          throw new Error(`SVG file not found: ${filePath}`);
        }

        let svgContent = fs.readFileSync(filePath, "utf-8").trim();
        const otherAttrs = attrStr.replace(/src=["']([^"']+)["']/i, "").trim();

        if (otherAttrs) {
          svgContent = svgContent.replace(/<svg\b([^>]*)>/i, `<svg$1 ${otherAttrs}>`);
        }

        return svgContent;
      });
    },
  };
}

export default defineConfig({
  plugins: [inlineSvgPlugin(), viteSingleFile()],
  test: {
    environment: "jsdom",
    coverage: {
      provider: "v8",
      reporter: ["text", "json", "html"],
      thresholds: {
        lines: 80,
        statements: 80,
        functions: 80,
      },
    },
  },
  build: {
    target: "esnext",
    assetsInlineLimit: 100000000,
    cssCodeSplit: false,
    rollupOptions: {
      output: {
        inlineDynamicImports: true,
      },
    },
  },
});
