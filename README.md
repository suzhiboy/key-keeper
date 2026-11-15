# Key Keeper · Account Vault

**English** · [简体中文](README.zh-CN.md)

Key Keeper is a local Windows vault for login credentials, API keys, two-factor authentication (2FA) secrets, and personal card or redemption codes. You can leave any field blank and fill it in later. The app supports tags, search, manual light and dark themes, bulk imports, and encrypted backups.

In this first release, a vault can hold up to 1,000 records.

## Download

**[Download Key Keeper for Windows (x64)](https://github.com/suzhiboy/key-keeper/releases/latest/download/Key-Keeper-Windows-x64-Setup.exe)** · [All releases](https://github.com/suzhiboy/key-keeper/releases)

Run the downloaded `.exe` to install Key Keeper. The installer and app interface are in Simplified Chinese. The installer is unsigned and may need an internet connection to install WebView2. The app runs offline.

## Usage

On first launch, choose a master password of at least 12 characters and keep it somewhere safe. There is no password reset or recovery.

For a login record, fill in any combination of platform, username, email address, phone number, and password. You can also save drafts for API keys, card codes, and 2FA. Generating a 2FA code requires a valid secret. Key Keeper accepts Base32 secrets and `otpauth://totp` links.

Passwords, secrets, and redemption codes are hidden by default. After 30 seconds, Key Keeper clears the clipboard if it still holds the value copied by the app. This does not clear Windows clipboard history or entries saved by third-party clipboard managers. The vault locks after five minutes of inactivity in Key Keeper, when Windows locks, or when the system goes to sleep. Activity in other apps does not reset the timer.

You can import TXT, CSV, XLS, and XLSX files. Choose the record type and file, configure the delimiter, text encoding, worksheet, and field mappings as applicable, then review the preview before confirming. For a TXT file with one code per line, map each line to the redemption code field. Fix rows with errors in the source file or field mappings, or explicitly choose to skip them. Exact duplicates are skipped by default. If a record has the same platform and account but different contents, it is kept as a new record. Store account identifiers and long card numbers as text in Excel: Key Keeper cannot recover leading zeros or digits already lost in the source file.

## Data and backups

The vault is stored at `com.keykeeper.desktop/vault.kkv` under the current Windows user's app data directory. The entire vault, including names, tags, and notes, is encrypted with AES-256-GCM using a key derived with Argon2id. The theme preference contains no secrets and is stored separately.

Backups are self-contained, encrypted `.kkv` files. To restore a backup, use the master password that was in effect when it was created. Changing the master password does not alter existing backups. Restoring a backup replaces the entire current vault, so back up any data you want to keep before confirming. Key Keeper does not upload your data or back it up to the cloud.

## Development

See the [development guide](docs/development.md) for build commands, checks, and desktop test coverage.
