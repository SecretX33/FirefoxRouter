# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

FirefoxRouter is a Windows utility written in Rust that acts as a default browser proxy. When registered as the system's default browser, it intercepts URL opens and routes them to Firefox using the currently active Firefox profile (detected via running processes).

**Purpose:** Firefox's native default browser registration always opens URLs in the default profile, even when you're actively using a different profile. FirefoxRouter solves this by detecting which Firefox profile is currently running and routing URLs there instead.

## Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build (size-optimized with LTO)
cargo clippy             # Lint
cargo fmt                # Format
```

No test suite exists.

## Architecture

Single-binary Rust application (~175 lines in `src/main.rs`). The entry point dispatches on CLI args:

- `--register` — Writes Windows registry entries to register as a browser (ProgIDs, StartMenuInternet, RegisteredApplications under HKCU)
- `--unregister` — Removes those registry entries
- Any other args — Treated as URLs to open in Firefox (`handle_link`)

**URL routing flow** (`handle_link`):
1. Uses `sysinfo` to enumerate running processes and find `firefox.exe` instances
2. Extracts `-profile` or `-P` flags from each process's command line arguments
3. If a profile is found, opens the URL in that profile via `firefox.exe -P <profile> -url <url>`
4. Falls back to opening without a profile flag (Firefox's default profile)

**Firefox discovery** (`find_firefox`): Checks `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\firefox.exe`, falls back to bare `firefox.exe` (PATH lookup).

**Supporting files**:
- `src/log_macro.rs` — `log!` (always prints) and `debug_log!` (debug builds only) macros
- `build.rs` — Embeds `icon.ico` into the Windows executable via `winres`

## Key Constraints

- Windows-only (`#[cfg(windows)]` guards, registry access)
- The release binary hides the console window (`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`)
- Release profile optimizes for binary size (`opt-level = 's'`, LTO enabled)
- Dependencies: `sysinfo` (process enumeration), `winreg` (Windows registry access), `color-eyre` (error handling)
