// The ESLint configuration `sloptopop collect` passes with --config (it
// replaces the project's own): eslint-plugin-sonarjs 2.0.4's recommended
// rules on JavaScript and TypeScript, nothing else. Written by sloptopop to
// .sloptopop/tools/eslint-sonarjs/; MIT OR Apache-2.0, like the rest of the
// sloptopop CLI. Imports resolve from this directory, never from the
// project's node_modules.
import sonarjs from "eslint-plugin-sonarjs";
import tsParser from "@typescript-eslint/parser";

const SOURCES = ["**/*.{js,mjs,cjs,jsx,ts,tsx,mts,cts}"];

export default [
  {
    // sloptopop's own files and generated code nobody wrote.
    ignores: [".sloptopop/**", "**/.next/**", "**/dist/**", "**/coverage/**", "**/*.min.js"],
  },
  {
    files: ["**/*.{js,mjs,cjs,jsx}"],
    languageOptions: { parserOptions: { ecmaFeatures: { jsx: true } } },
  },
  {
    // No type information (no parserOptions.project): rules that need it
    // turn themselves off, and nothing from the project is built or run.
    files: ["**/*.{ts,tsx,mts,cts}"],
    languageOptions: { parser: tsParser },
  },
  { ...sonarjs.configs.recommended, files: SOURCES },
  {
    // Directives for rules of other plugins are not this run's business.
    linterOptions: { reportUnusedDisableDirectives: "off" },
  },
];
