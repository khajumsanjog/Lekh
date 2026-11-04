# लेख · Lekh (`lk`)

[![Latest Release](https://img.shields.io/github/v/release/khajumsanjog/Lekh?label=release&color=d9480f)](https://github.com/khajumsanjog/Lekh/releases/latest)
[![Rust 1.74+](https://img.shields.io/badge/rust-1.74%2B-orange.svg)](https://www.rust-lang.org)
[![Unsafe Forbidden](https://img.shields.io/badge/unsafe-forbidden-0f8f63.svg)](src/lib.rs)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**Lekh** (*लेख* — "writing") is a small, fast, crash-proof terminal text editor written in 100% safe Rust. It combines **GNU Nano's zero-learning-curve friendliness** with modern capabilities: 16-language syntax highlighting, 5 True-Color themes, mouse support, live smart-case search, group-merged undo/redo, and atomic saves.

Installing Lekh gives you two commands built from the same binary codebase—`lekh` (full name) and `lk` (fast 2-letter alias):

```sh
lk index.html          # open or create a file
lekh index.html        # identical full command
lk +120 src/main.rs    # open directly at line 120
```

---

## Highlights

- **Zero Modes, Instant Muscle Memory** — Familiar `Ctrl+S` save, `Ctrl+Q` quit, `Ctrl+Z` / `Ctrl+Y` undo/redo, and `Ctrl+C` / `Ctrl+X` / `Ctrl+V` clipboard (with automatic **OSC 52** host clipboard sync over SSH).
- **Full Mouse Support** — Click to place the cursor, drag to select, double-click to select a word, triple-click to select a line, and scroll with the mouse wheel.
- **5 Crisp Transparent-Canvas Themes (`Ctrl+T`)** — Cycle through **Default, Dracula, Nord, OneDark, and Gruvbox** in 24-bit RGB while keeping your terminal's native background crisp and clear. Includes matching `()[]{}` bracket highlights and a right-edge minimap scrollbar.
- **16 Languages Out of the Box** — Compiled directly into the binary with per-line token caching—no regex files, no external grammars, and embedded `<script>` / `<style>` highlighting inside HTML.
- **Hybrid Relative Line Numbers (`Ctrl+N`)** — Cycle between **Absolute**, Vim-style **Relative** (showing exact line number on the active cursor row), and **Off**.
- **Live Incremental Search & Replace** — `Ctrl+F` jumps to matches as you type (smart-case: lowercase queries ignore case; uppercase queries match case), `F3` / `Shift+F3` steps through matches, and `Ctrl+R` replaces all occurrences in a single undoable step.
- **0% Idle CPU & Tiny Footprint** — Sleeps in the OS kernel on `event::read()` until input arrives and flushes each frame in a single buffered write. Only **2 crate dependencies** (`crossterm` and `unicode-width`).

---

## Installation

### 1. Prebuilt Binaries (No Rust / Cargo Required)

Grab the latest precompiled archive or installer from **[GitHub Releases](https://github.com/khajumsanjog/Lekh/releases/latest)**.

#### Windows (PowerShell, Windows Terminal & `cmd.exe`)

**Option A — One-line PowerShell installer (Recommended):**
Installs **both** `lk.exe` and `lekh.exe` into `%LOCALAPPDATA%\Programs\Lekh` and registers them on your Windows User `PATH` so `lk` and `lekh` work from any folder in PowerShell, Windows Terminal, and `cmd.exe`:

```powershell
irm https://github.com/khajumsanjog/Lekh/releases/latest/download/install.ps1 | iex
```

**Option B — Graphical Setup `.exe`:**
Download and run **`lekh-windows-x86_64-setup.exe`** from **[GitHub Releases](https://github.com/khajumsanjog/Lekh/releases/latest)** (per-user install, no Administrator prompt required, includes uninstaller in Windows *Installed Apps*).

> **Windows tip:** Windows Terminal is recommended. Legacy `cmd.exe` lacks bracketed paste, so pasted multi-line blocks pass through auto-indent (press `Ctrl+P` first to toggle auto-indent off if needed).

#### macOS (Apple Silicon)

```sh
# Download from GitHub Releases, extract, and move onto PATH:
tar -xzf lekh-macos-arm64.tar.gz
xattr -d com.apple.quarantine lk lekh 2>/dev/null || true
chmod +x lk lekh
sudo mv lk lekh /usr/local/bin/

lk --version
```

> **macOS Terminal tip:** Enable *"Use Option as Meta key"* in **Terminal → Settings → Profiles → Keyboard** so `Option+Left/Right` jumps by word (`Ctrl+Arrow` is reserved by macOS Mission Control).

#### Linux (`x86_64`)

```sh
tar -xzf lekh-linux-x86_64.tar.gz
sudo install -m 755 lk lekh /usr/local/bin/

lk --version
```

---

### 2. Build from Source (Rust 1.74+)

Requires [Rust 1.74+](https://rustup.rs). Compiles an LTO-optimized, stripped binary and installs **both** `lekh` and `lk` into `~/.cargo/bin`:

```sh
git clone https://github.com/khajumsanjog/Lekh.git
cd Lekh
cargo install --path .
```

Or install directly from Git without cloning manually:

```sh
cargo install --git https://github.com/khajumsanjog/Lekh.git
```

---

## Command-Line Usage

```text
USAGE
    lk   [OPTIONS] [+LINE] [FILE]
    lekh [OPTIONS] [+LINE] [FILE]

OPTIONS
    -h, --help       Show command-line help
    -V, --version    Show version
    --               Treat all following arguments as a file name

ENVIRONMENT VARIABLES
    LEKH_MAX_MB      Maximum file size to open in MB (default: 64)
    LEKH_NO_OSC52    Disable OSC 52 system clipboard sync when set
```

Examples:

```sh
lk notes.md                     # open existing file or start a new buffer
lk +42 src/editor.rs            # open at line 42
LEKH_MAX_MB=256 lk server.log   # raise safety size limit to 256 MB
lk -- -weird-filename.txt       # open a file whose name starts with '-'
```

---

## Keyboard & Mouse Shortcuts

Press **`Ctrl+G`** or **`F1`** inside the editor at any time to toggle the built-in help overlay.

| Category | Shortcut | Action |
| :--- | :--- | :--- |
| **File** | `Ctrl+S` | Atomic save |
| | `Ctrl+O` | Save As (prompt for file path) |
| | `Ctrl+Q` | Quit (prompts if there are unsaved changes) |
| **Edit** | `Ctrl+Z` / `Ctrl+Y` | Undo / Redo (consecutive typing & deletions merge automatically) |
| | `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | Copy (plus OSC 52 sync), Cut, Paste |
| | `Ctrl+K` | Cut entire current line |
| | `Ctrl+D` | Duplicate current line |
| | `Tab` / `Shift+Tab` | Indent / Dedent line or multi-line selection |
| | `Ctrl+Backspace` / `Ctrl+Del` | Delete word left / word right |
| **Select** | `Shift + Arrows / Home / End` | Extend selection |
| | `Ctrl+A` | Select entire buffer |
| | Mouse click / drag | Place cursor / drag selection |
| | Double-click / Triple-click | Select word / select entire line |
| **Move** | `Arrows`, `PgUp`, `PgDn` | Move cursor / scroll page |
| | `Home` / `End` | Smart `Home` (toggles first non-blank ↔ column 0) / end of line |
| | `Ctrl+Left` / `Ctrl+Right` | Jump left / right by word (`Option+Left/Right` on macOS) |
| | `Ctrl+Home` / `Ctrl+End` | Jump to start / end of file |
| | `Ctrl+L` | Go to `line` or `line:col` |
| **Find** | `Ctrl+F` | Live incremental search (`Enter`/`↓` next, `↑` prev, `Esc` close) |
| | `F3` / `Shift+F3` | Next / previous search match |
| | `Ctrl+R` | Find & Replace All (single undoable step) |
| **View** | `Ctrl+T` | Cycle color themes (Default, Dracula, Nord, OneDark, Gruvbox) |
| | `Ctrl+N` | Cycle line numbers (Absolute → Relative → Off) |
| | `Ctrl+P` | Toggle smart auto-indent (useful before pasting in legacy consoles) |
| | `Ctrl+G` / `F1` | Toggle in-editor shortcut help |

---

## Highlighted Languages (16 Built-in)

| Language | Extensions / Filenames |
| :--- | :--- |
| **Rust** | `.rs` |
| **Python** | `.py`, `.pyw`, `.pyi` |
| **JavaScript / TypeScript** | `.js`, `.mjs`, `.cjs`, `.jsx`, `.ts`, `.mts`, `.tsx` |
| **C / C++** | `.c`, `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh` |
| **Java / C# / Kotlin / Swift** | `.java`, `.cs`, `.kt`, `.kts`, `.swift`, `.scala` |
| **Go** | `.go` |
| **Shell** | `.sh`, `.bash`, `.zsh`, `bashrc`, `zshrc`, `Dockerfile`, `Makefile` |
| **HTML / XML** | `.html`, `.htm`, `.xhtml`, `.xml`, `.svg`, `.vue`, `.svelte` *(with embedded JS & CSS)* |
| **CSS / SCSS** | `.css`, `.scss`, `.less` |
| **JSON** | `.json`, `.jsonc`, `.jsonl` |
| **TOML / YAML / INI** | `.toml`, `.yaml`, `.yml`, `.ini`, `.cfg`, `.conf`, `.env`, `Cargo.lock` |
| **Markdown** | `.md`, `.markdown` |
| **Lua** | `.lua` |
| **Ruby** | `.rb`, `Gemfile`, `Rakefile` |
| **SQL** | `.sql` |
| **Zig** | `.zig` |

Adding a new language requires adding a single `static Lang` definition in [`src/syntax.rs`](src/syntax.rs).

---

## Security & Reliability Design

- **100% Safe Rust (`#![forbid(unsafe_code)]`)** — Zero `unsafe` blocks, no shell-outs, no network calls, no plugin execution, and no executed config scripts.
- **Crash-Proof 4-Step Atomic Saves ([`src/fileio.rs`](src/fileio.rs))** — Writes to a private `0600` sibling temp file opened with `O_EXCL` (refusing planted symlinks) → copies original permissions → calls hardware `sync_all()` (`fsync`) on the file and parent directory → atomically swaps via POSIX `rename()`. A crash or power loss never leaves a truncated half-written file. Symlinks are resolved so saving updates the target while keeping the symlink intact.
- **External Modification Guard** — Checks file `mtime` before saving; if another process modified the file on disk after you opened it, Lekh warns and asks before overwriting.
- **Trojan-Source Defense (CVE-2021-42574)** — Invisible and bidirectional Unicode override characters are rendered as bold red `?` markers and trigger an immediate status-bar warning on load.
- **Terminal Escape-Sequence Armor** — File contents, filenames, and pasted text can never emit raw ANSI escape sequences to your terminal; control bytes are safely drawn as `^X`.
- **Hostile-Input Protection** — Cleanly refuses directories, FIFOs, and device nodes (e.g., `lk /dev/zero`), refuses binary / non-UTF-8 files rather than corrupting them, enforces a configurable size cap (`LEKH_MAX_MB`, default `64` MB), and restores raw mode & alternate screen on both normal exit and panic via a RAII `TermGuard`.
- **Format Preservation** — Preserves UTF-8 BOM, `LF` vs `CRLF` line endings, and whether the file ends with a trailing newline.

---

## Project Layout

```text
src/
├── main.rs       # `lekh` binary entry point
├── bin/lk.rs     # `lk` short-alias binary entry point
├── lib.rs        # CLI argument parser, RAII TermGuard, 0%-idle event loop
├── buffer.rs     # Vec<char> text storage & reversible algebraic undo/redo log
├── editor.rs     # Cursor math, selection, clipboard, smart-case search, key/mouse handlers
├── syntax.rs     # Zero-regex single-pass tokenizer for 16 languages
├── ui.rs         # Single-flush viewport renderer, 5 themes, minimap scrollbar
└── fileio.rs     # O_EXCL + fsync + atomic rename & hostile-file guards
```

---

## Honest Limitations

- **Single-buffer view:** Edits one file per terminal session (no tabs or split panes yet).
- **UTF-8 text only:** Non-UTF-8 or NUL-containing binary files are refused rather than lossily converted. Mixed line endings are normalized to the first line-ending style encountered.
- **`Vec<char>` line representation:** Simple and fast (`O(1)` column indexing), using ~4 bytes per character in RAM (~60 MB for a 12 MB source file; ~2 MB for an empty session). A rope structure would be the next step for multi-hundred-megabyte files.
- **Signal handling:** No `SIGTERM` / `SIGHUP` emergency swap-file dump yet (kept dependency-free without a signal crate).
- **Complex script shaping:** Wide and combining characters are measured via `unicode-width`, while ligatures and RTL shaping depend on your terminal emulator.

---

## Development & Tests

```sh
cargo test              # runs unit tests across buffer/undo, atomic file I/O, and syntax highlighting
cargo build --release   # outputs target/release/lekh and target/release/lk
```

---

## License

Released under the [MIT License](LICENSE).
