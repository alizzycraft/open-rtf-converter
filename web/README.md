# Open RTF Converter website

This is the static Angular 22 front end for the Rust converter. It uses signals, explicit zoneless change detection, and a module worker so CPU-heavy WebAssembly conversion stays off the UI thread. Files are processed locally and are never uploaded.

## Local development

Install Node.js 22, the repository's pinned Rust toolchain, and the CLI matching the `wasm-bindgen` dependency:

```powershell
cargo install wasm-bindgen-cli --version 0.2.127 --locked
npm ci
npm run build:wasm
npm start
```

Open `http://localhost:4200`. The WASM output under `public/wasm` is generated and ignored by Git.

## Verification and production builds

```powershell
npm test -- --watch=false
npm run build
npm run build:pages
```

- `build` generates the WASM bindings and a root-hosted static Angular build.
- `build:pages` uses `/open-rtf-converter/` as the base path for GitHub project Pages.
- Static files are emitted to `dist/web/browser`.

The repository's Pages workflow installs the pinned binding CLI, builds the complete app, uploads the static artifact, and deploys it. In GitHub repository settings, select **GitHub Actions** as the Pages source.
