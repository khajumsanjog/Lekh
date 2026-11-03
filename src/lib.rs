//! Lekh (`lekh` / `lk`): a small, fast, safe terminal text editor.
#![forbid(unsafe_code)]

mod buffer;
mod editor;
mod fileio;
mod syntax;
mod ui;

use crossterm::{
    cursor::SetCursorStyle,
    event::{self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use editor::Editor;
use std::ffi::OsString;
use std::io::{self, BufWriter, IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_MAX_MB: u64 = 64;

const USAGE: &str = "\
lekh - a small, fast, safe terminal text editor   (short command: lk)

USAGE
    lekh [OPTIONS] [+LINE] [FILE]
    lk   [OPTIONS] [+LINE] [FILE]

OPTIONS
    -h, --help       show this help
    -V, --version    show version
    --               treat everything after as a file name

ENVIRONMENT
    LEKH_MAX_MB      largest file that will be opened (default 64)
    LEKH_NO_OSC52    do not copy to the system clipboard via OSC 52

Inside the editor press Ctrl+G (or F1) for the key list.
";

/// Puts the terminal back, whatever happens (also called from the panic hook).
fn restore_terminal() {
    let mut out = io::stdout();
    let _ = execute!(out, DisableBracketedPaste, DisableMouseCapture, SetCursorStyle::DefaultUserShape, LeaveAlternateScreen);
    let _ = disable_raw_mode();
    let _ = out.flush();
}

struct TermGuard;

impl TermGuard {
    fn enter() -> io::Result<TermGuard> {
        enable_raw_mode()?;
        let guard = TermGuard; // from here on Drop restores the terminal
        let mut out = io::stdout();
        execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
        let _ = execute!(out, EnableBracketedPaste); // not supported by very old Windows consoles
        Ok(guard)
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn event_loop(ed: &mut Editor) -> io::Result<()> {
    let mut out = BufWriter::with_capacity(1 << 16, io::stdout());
    let (w, h) = terminal::size()?;
    ed.resize(w, h);
    ed.render(&mut out)?;
    loop {
        // Sleeps in the kernel until something happens: 0% CPU when idle.
        let mut ev = event::read()?;
        let mut redraw = false;
        loop {
            redraw |= ed.handle_event(ev);
            if ed.quit {
                return Ok(());
            }
            // Drain queued input (fast typing / pasting) before drawing once.
            if event::poll(Duration::ZERO)? {
                ev = event::read()?;
            } else {
                break;
            }
        }
        if redraw {
            ed.render(&mut out)?;
        }
    }
}

pub fn run() -> i32 {
    let mut file: Option<OsString> = None;
    let mut line: Option<usize> = None;
    let mut options_done = false;
    for a in std::env::args_os().skip(1) {
        let s = a.to_string_lossy().into_owned();
        if !options_done {
            match s.as_str() {
                "--" => {
                    options_done = true;
                    continue;
                }
                "-h" | "--help" => {
                    print!("{}", USAGE);
                    return 0;
                }
                "-V" | "--version" => {
                    println!("lekh {}", VERSION);
                    return 0;
                }
                _ => {}
            }
            if let Some(n) = s.strip_prefix('+').and_then(|n| n.parse::<usize>().ok()) {
                line = Some(n);
                continue;
            }
            if s.starts_with('-') && s.len() > 1 {
                eprintln!("lekh: unknown option '{}'\n\n{}", editor::sanitize(&s), USAGE);
                return 2;
            }
        }
        if file.is_some() {
            eprintln!("lekh: only one file at a time (multi-file tabs are not implemented yet)");
            return 2;
        }
        file = Some(a);
    }

    if !io::stdout().is_terminal() {
        eprintln!("lekh: standard output is not a terminal");
        return 1;
    }

    let max_mb = std::env::var("LEKH_MAX_MB")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_MAX_MB);

    let mut ed = match Editor::new(file.map(PathBuf::from), max_mb.saturating_mul(1024 * 1024)) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("lekh: {}", e);
            return 1;
        }
    };
    if let Some(l) = line {
        ed.goto_line(l);
    }

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        prev(info);
    }));

    let guard = match TermGuard::enter() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("lekh: cannot initialise terminal: {}", e);
            return 1;
        }
    };
    let result = event_loop(&mut ed);
    drop(guard);
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("lekh: {}", e);
            1
        }
    }
}
