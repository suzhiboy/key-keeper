# Development · 开发指南

Key Keeper uses React, TypeScript, and Vite in `src/` and Tauri and Rust in `src-tauri/`. The [RPC contract](api.md) describes the interface between them. The editable app icon is in `brand/`.

## Build · 构建

Install Node.js, the stable Rust MSVC toolchain, the **Desktop development with C++** workload in Visual Studio 2022 Build Tools, the Windows SDK, and WebView2. Reopen your terminal after installing Rust so it picks up Cargo on the PATH.

```powershell
npm ci
npm run tauri dev
```

Build the Windows x64 installer:

```powershell
npm run tauri build -- --target x86_64-pc-windows-msvc
```

Find the NSIS installer in `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`. Upload it to the GitHub Release as `Key-Keeper-Windows-x64-Setup.exe` so the download links in both READMEs continue to work. Users download later versions from GitHub Releases.

## Checks · 检查

```powershell
npm test
npm run typecheck
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

In debug builds, use `KEY_KEEPER_DATA_DIR` to point the app at synthetic test data. Release builds use the normal app data directory. Use synthetic passwords and import files for testing.

On November 15, 2026, automated checks passed and the Windows debug build was used to verify vault creation, saving a partial login record, unlocking after a restart, restoring a minimized window, and the light and dark layouts. Full desktop checks for imports, backup restoration, timed clipboard clearing, and locking on system lock or sleep were still pending. Automated tests cover the related data handling and locking logic.

## Repository conventions · 仓库约定

Use `feat`, `fix`, `test`, `docs`, or `build` as commit prefixes. Track dependency lockfiles and the synthetic spreadsheets used by the import tests. Keep real vaults, backups, import data, environment secrets, and installers out of Git. Use the ignored `private-data/` directory for local data when needed.
