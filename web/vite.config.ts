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
      let transformed = html.replace(/<svg-icon\s+([^>]*?)(?:><\/svg-icon>|\/>)/gi, (_, attrStr) => {
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

      // Inline favicon SVG as a Data URI to avoid external hashed asset files
      const logoSvgPath = path.resolve(import.meta.dirname, "src/assets/icons/logo.svg");
      if (fs.existsSync(logoSvgPath)) {
        const svgContent = fs.readFileSync(logoSvgPath, "utf-8").trim();
        const svgBase64 = Buffer.from(svgContent).toString("base64");
        transformed = transformed.replace(
          /<link\s+[^>]*rel=["']icon["'][^>]*>/gi,
          `<link rel="icon" type="image/svg+xml" href="data:image/svg+xml;base64,${svgBase64}">`
        );
      }

      // Inline fallback favicon.ico as a Data URI
      const icoPath = path.resolve(import.meta.dirname, "favicon.ico");
      if (fs.existsSync(icoPath)) {
        const icoBase64 = fs.readFileSync(icoPath).toString("base64");
        transformed = transformed.replace(
          /<link\s+[^>]*rel=["']alternate icon["'][^>]*>/gi,
          `<link rel="alternate icon" href="data:image/x-icon;base64,${icoBase64}">`
        );
      }

      return transformed;
    },
  };
}

export default defineConfig({
  plugins: [inlineSvgPlugin(), viteSingleFile()],
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
    exclude: ["e2e/**", "node_modules/**"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.ts"],
      exclude: ["src/**/*.test.ts", "src/main.ts"],
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
