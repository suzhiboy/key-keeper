# Key Keeper v1

Approved 2026-11-15. Windows offline personal vault, <=1000 records. Tauri 2 + React/TypeScript/Vite with Rust core; mobile later, no cloud/backend. Manual light/dark themes. All record fields optional; incomplete records save. Login: platform, username, email, phone, password, website, notes, optional TOTP. Also API keys, standalone TOTP and personal card codes with used/expiry. CRUD, tags/search, masked secrets, clipboard copy. TOTP validation only for generation; incomplete configurations save.

Argon2id (64MiB,t3,p1) + AES256GCM whole-file encryption. Random salt/nonce, authenticated header, atomic transactions, single instance. Master password >=12 chars, change password, 5-minute app idle lock, OS session lock/suspend, clipboard clear after30s if unchanged. Rust controls secrets and session; late results invalidated on lock.

TXT/CSV/XLS/XLSX import: type, encoding/delimiter/sheet, field mapping, preview/row errors/duplicates then atomic commit. Fully identical duplicates skipped by default; no overwrites. One-column TXT card maps secretCode. UTF8/BOM and GB18030. Preserve text credential data. No plaintext temp files.

Encrypted backup export and restore from locked/welcome/settings. Validate with backup password, inspect count, explicitly confirm replacement, atomic replace and lock. Old backups retain old password.

## Tasks
1. Build tooling and contracts.
2. Rust vault, session, records, TOTP and backup, tests.
3. React UI and key interaction tests.
4. Import preview/commit flow and tests.
5. Native integration, review, checks and installer.

## Repository
main baseline; feat/key-keeper-v1 implementation. Commit cohesive validated changes as build/feat/fix/test/docs; no pushes. Never track vaults, secrets, real import files or outputs. Keep lockfiles. Native execution in empty dedicated project directory; no extra worktree needed.

## Acceptance
Partial login saving; crypto tamper/wrong password/write failure; RFC6238; four-type CRUD; all import formats/Chinese/leading zeros/quotes/duplicate/cancel; empty-directory restore; idle/session lock; theme; actual Windows smoke; cargo test/clippy/fmt, npm tests/typecheck/build, Tauri installer.
