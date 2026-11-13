# Frontend Humcase

React 19, TypeScript, Vite, Tailwind, Tiptap and Radix primitives, hosted in the Tauri WebView.

Read [AGENTS](../AGENTS.md) and the [documentation index](../docs/README.md).

- `npm test`: Vitest/jsdom tests.
- `npm run build`: TypeScript then Vite.
- From the root: `npm run tauri dev` for the native app; `npm run update-exe` to deliver.

The Vite server alone does not validate Tauri commands, Rust persistence or native rendering. Modify the existing application, not a copy of its screens.
