# Lekh (`lk`)

A small, fast, safe terminal text editor written in Rust. Nano-easy, with mouse
support, syntax colours, live search, undo/redo and more.

    lk index.html        # short command
    lekh index.html      # full name
    lk +120 main.rs      # open at line 120

## Download & Install

### Prebuilt Binaries (Recommended)
Download the latest archive for your OS from **[GitHub Releases](https://github.com/khajumsanjog/Lekh/releases/latest)**, extract `lk` and `lekh`, and place them on your `PATH`.

### Build from Source

    cargo build --release          # binaries: target/release/lekh and target/release/lk
    cargo install --path .         # installs BOTH `lekh` and `lk` onto your PATH

Needs Rust 1.74+ (https://rustup.rs). Works on Linux, macOS, and Windows
(Windows Terminal / PowerShell / cmd). On Windows, Windows Terminal is
recommended; legacy cmd.exe has no bracketed paste, so pasting there goes
through auto-indent (toggle it off with Ctrl+P first).

## Keys  (Ctrl+G or F1 shows them inside the editor)

| | |
|---|---|
| File | `Ctrl+S` save, `Ctrl+O` save as, `Ctrl+Q` quit |
| Edit | `Ctrl+Z` undo, `Ctrl+Y` redo, `Ctrl+C/X/V`, `Ctrl+K` cut line, `Ctrl+D` duplicate line, `Ctrl+A` select all |
| Indent | `Tab` / `Shift+Tab` (works on multi-line selections) |
| Find | `Ctrl+F` live search (Enter/Down next, Up prev, Esc done), `F3`/`Shift+F3`, `Ctrl+R` replace all |
| Move | arrows, Home (smart), End, PgUp/PgDn, `Ctrl+←/→` word, `Ctrl+Home/End`, `Ctrl+L` go to line |
| Select | `Shift`+movement, mouse drag, double-click word, triple-click line |
| Mouse | click to place the cursor, wheel to scroll |
| View | `Ctrl+N` line numbers (cycles Absolute / Relative / Off), `Ctrl+P` auto-indent, `Ctrl+T` themes |

macOS Terminal: use Option+Arrow for word movement and enable
"Use Option as Meta key"; Ctrl+Arrow is taken by Mission Control.
Search is smart-case: a lowercase query ignores case.

## Languages highlighted

Rust, Python, JavaScript/TypeScript, C/C++, Java/C#/Kotlin/Swift, Go, Shell,
HTML/XML (with embedded `<script>` and `<style>`), CSS, JSON, TOML/YAML/INI,
Markdown, Lua, Ruby, SQL, Zig. Colours: keywords, types, functions, variables, strings, numbers,
constants, macros, comments, tags, attributes.
Add a language = add one `static Lang` in `src/syntax.rs`.

## Security design

* `#![forbid(unsafe_code)]`; no shell-outs, no network, no plugins, no config
  files executed, two small dependencies (`crossterm`, `unicode-width`).
* **Terminal-escape injection**: file contents and pasted text can never send
  escape sequences to your terminal; control characters are drawn as `^[`.
* **Trojan-Source defence**: invisible and bidirectional Unicode characters are
  drawn as a red `?` and a warning is shown when a file contains them.
* **Atomic saves**: temp file (`O_EXCL`) -> fsync -> rename. A crash or power
  cut never leaves a half-written file. Permissions are preserved (the temp
  file is private `0600` until they are copied), symlinks are followed so the
  link stays a link, read-only files are respected.
* **Overwrite protection**: if the file changed on disk after you opened it,
  Lekh asks before overwriting. Quitting with unsaved changes asks too.
* **Hostile input**: refuses directories, devices and pipes (`lk /dev/zero`),
  binary/non-UTF-8 files (so they are not corrupted), and files over 64 MB
  (`LEKH_MAX_MB`). Every buffer operation bounds-checks; the terminal is
  restored on exit and on panic.
* Line endings (LF/CRLF), BOM and the final newline are preserved.

## Honest limitations

* One file at a time (no tabs/splits yet). Non-UTF-8 files are refused rather
  than converted. Mixed line endings are normalised to the first style found.
* Text is stored as `Vec<char>` per line: simple and fast, but ~4 bytes/char
  (a 12 MB file uses ~60 MB RAM). An empty editor uses ~2 MB.
  A rope (`ropey`) would be the next step for giant files.
* No SIGTERM/SIGHUP emergency-save yet (would need a signal crate).
* Wide/combining Unicode is handled via `unicode-width`, but complex scripts
  (ligatures, RTL shaping) depend on your terminal.

## Tests

    cargo test        # buffer/undo, secure file I/O, syntax highlighter

## License

Licensed under the [MIT License](LICENSE).
