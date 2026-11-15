# Key Keeper · Account Vault

**English** · [简体中文](README.zh-CN.md)

Key Keeper is a local Windows vault for login credentials, API keys, two-factor authentication (2FA) secrets, and personal card or redemption codes. You can leave any field blank and fill it in later. The app supports tags, search, manual light and dark themes, bulk imports, and encrypted backups.

In this first release, a vault can hold up to 1,000 records.

## Development and build

You'll need Node.js, the stable Rust MSVC toolchain, the **Desktop development with C++** workload in Visual Studio 2022 Build Tools, the Windows SDK, and WebView2.

```powershell
npm ci
npm run tauri dev
```

After installing Rust for the first time, reopen your terminal to refresh your PATH. To build the Windows installer:

```powershell
npm run tauri build
```

The installer is generated in `src-tauri/target/release/bundle/nsis/`. The app runs offline. If WebView2 is missing, the installer needs to install the runtime. The project has no commercial code signing or automatic update service configured.

## Usage

On first launch, choose a master password of at least 12 characters and keep it somewhere safe. There is no password reset or recovery.

For a login record, fill in any combination of platform, username, email address, phone number, and password. You can also save drafts for API keys, card codes, and 2FA. Generating a 2FA code requires a valid secret. Key Keeper accepts Base32 secrets and `otpauth://totp` links.

Passwords, secrets, and redemption codes are hidden by default. After 30 seconds, Key Keeper clears the clipboard if it still holds the value copied by the app. This does not clear Windows clipboard history or entries saved by third-party clipboard managers. The vault locks after five minutes of inactivity in Key Keeper, when Windows locks, or when the system goes to sleep. Activity in other apps does not reset the timer.

You can import TXT, CSV, XLS, and XLSX files. Choose the record type and file, configure the delimiter, text encoding, worksheet, and field mappings as applicable, then review the preview before confirming. For a TXT file with one code per line, map each line to the redemption code field. Fix rows with errors in the source file or field mappings, or explicitly choose to skip them. Exact duplicates are skipped by default. If a record has the same platform and account but different contents, it is kept as a new record. Store account identifiers and long card numbers as text in Excel: Key Keeper cannot recover leading zeros or digits already lost in the source file.

## Data and backups

The vault is stored at `com.keykeeper.desktop/vault.kkv` under the current Windows user's app data directory. The entire vault, including names, tags, and notes, is encrypted with AES-256-GCM using a key derived with Argon2id. The theme preference contains no secrets and is stored separately.

Backups are self-contained, encrypted `.kkv` files. To restore a backup, use the master password that was in effect when it was created. Changing the master password does not alter existing backups. Restoring a backup replaces the entire current vault, so back up any data you want to keep before confirming. Key Keeper does not upload your data or back it up to the cloud.

## Checks

```powershell
npm test
npm run typecheck
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
```

In debug builds, set `KEY_KEEPER_DATA_DIR` to a directory for synthetic test data. Release builds ignore this variable. Never use real passwords or import files in tests.

## Repository conventions

`main` holds the initial baseline. Development takes place on `feat/key-keeper-v1`. Use `feat`, `fix`, `test`, `docs`, or `build` as commit prefixes. Commit dependency lockfiles. Keep real vaults, backups, import files, environment secrets, and build output out of Git. If you need to use local data temporarily, put it in the ignored `private-data/` directory.

This release supports one user and one vault on Windows. It does not include mobile apps, cloud sync, browser autofill, attachments, or team sharing.

## Verification status

November 15, 2026: All four frontend tests, TypeScript checks, the production build, Rust unit and integration tests, Clippy with warnings treated as errors, and formatting checks passed. The Windows x64 NSIS installer was built.

The Windows debug build was used to verify vault creation, saving a record with only the platform and email filled in, unlocking after a restart, and restoring a minimized window by launching the app again. Light and dark layouts were also checked. Desktop testing was paused before full end-to-end verification of imports, backup restoration, timed clipboard clearing, and locking on Windows lock or sleep. Automated tests cover the related data handling and locking logic. The installer has not been run on the test machine.
