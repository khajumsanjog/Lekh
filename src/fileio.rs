//! Safe file loading and saving.
//!
//! Security / safety properties:
//!  * refuses directories, devices, FIFOs (e.g. `lk /dev/zero` cannot hang or eat RAM)
//!  * hard size limit (default 64 MB, `LEKH_MAX_MB` to change)
//!  * refuses non-UTF-8 / binary data instead of silently corrupting it
//!  * saves ATOMICALLY: write temp file (O_EXCL) -> fsync -> rename over target,
//!    so a crash or power loss can never leave a half-written file
//!  * keeps the original permissions (temp file is 0600 until perms are copied)
//!  * follows symlinks so saving edits the target and keeps the link
//!  * respects read-only files

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Loaded {
    pub lines: Vec<Vec<char>>,
    pub crlf: bool,
    pub final_newline: bool,
    pub bom: bool,
    pub mtime: Option<SystemTime>,
    pub existed: bool,
    pub writable: bool,
}

impl Loaded {
    pub fn empty() -> Self {
        Loaded {
            lines: vec![Vec::new()],
            crlf: false,
            final_newline: true,
            bom: false,
            mtime: None,
            existed: false,
            writable: true,
        }
    }
}

pub fn load(path: &Path, max: u64) -> Result<Loaded, String> {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Loaded::empty()),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file() {
        return Err("not a regular file (directories, devices and pipes are refused)".into());
    }
    if meta.len() > max {
        return Err(format!(
            "file is too large ({} MB, limit {} MB; set LEKH_MAX_MB to raise it)",
            meta.len() / 1_048_576,
            max / 1_048_576
        ));
    }
    let f = File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    f.take(max + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max {
        return Err("file grew past the size limit while reading".into());
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| "not valid UTF-8 text (binary or another encoding); refusing to open it so it is not corrupted".to_string())?;
    if text.contains('\0') {
        return Err("contains NUL bytes (binary file); refusing to open".into());
    }

    let (bom, body) = match text.strip_prefix('\u{feff}') {
        Some(r) => (true, r),
        None => (false, text.as_str()),
    };
    let crlf = body.find('\n').map_or(false, |i| i > 0 && body.as_bytes()[i - 1] == b'\r');
    let ends_nl = body.ends_with('\n');
    let mut lines: Vec<Vec<char>> = body
        .split('\n')
        .map(|l| {
            let l = if crlf { l.strip_suffix('\r').unwrap_or(l) } else { l };
            l.chars().collect()
        })
        .collect();
    if ends_nl {
        lines.pop();
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    let writable = OpenOptions::new().write(true).open(path).is_ok();
    Ok(Loaded {
        lines,
        crlf,
        final_newline: body.is_empty() || ends_nl,
        bom,
        mtime: meta.modified().ok(),
        existed: true,
        writable,
    })
}

pub fn mtime_of(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Atomic, permission-preserving save.
pub fn save(path: &Path, data: &[u8]) -> io::Result<()> {
    // Follow symlinks: edit the real file, keep the link.
    let target: PathBuf = match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => fs::canonicalize(path)?,
        _ => path.to_path_buf(),
    };
    let existing = match fs::metadata(&target) {
        Ok(m) => {
            if !m.is_file() {
                return Err(io::Error::new(io::ErrorKind::Other, "target is not a regular file"));
            }
            Some(m)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if existing.is_some() {
        // Honour read-only files (rename() would otherwise bypass the file's own mode).
        OpenOptions::new().write(true).open(&target)?;
    }

    let dir = match target.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let name: String = target
        .file_name()
        .map(|n| n.to_string_lossy().chars().take(100).collect())
        .unwrap_or_else(|| "file".into());
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let tmp = dir.join(format!(".{}.lekh-{}-{}.tmp", name, std::process::id(), nanos));

    let result = (|| -> io::Result<()> {
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true); // O_EXCL: never follows a planted symlink
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Existing file: start private (0600) so its contents are never briefly
            // world-readable. New file: normal 0666 & umask.
            opts.mode(if existing.is_some() { 0o600 } else { 0o666 });
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(data)?;
        if let Some(m) = &existing {
            fs::set_permissions(&tmp, m.permissions())?;
        }
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, &target)
    })();

    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    #[cfg(unix)]
    {
        if let Ok(d) = File::open(&dir) {
            let _ = d.sync_all();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("lekh-test-{}-{}", tag, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn roundtrip_and_no_temp_left() {
        let d = tmpdir("rt");
        let p = d.join("a.txt");
        save(&p, b"hello\r\nworld\r\n").unwrap();
        let l = load(&p, 1 << 20).unwrap();
        assert!(l.crlf && l.final_newline);
        assert_eq!(l.lines.len(), 2);
        assert_eq!(fs::read_dir(&d).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn refuses_binary_and_big_and_dirs() {
        let d = tmpdir("ref");
        let p = d.join("b.bin");
        fs::write(&p, [0xff, 0xfe, 0x00]).unwrap();
        assert!(load(&p, 1 << 20).is_err());
        fs::write(&p, "ok\0bad").unwrap();
        assert!(load(&p, 1 << 20).is_err());
        fs::write(&p, "x".repeat(100)).unwrap();
        assert!(load(&p, 10).is_err());
        assert!(load(&d, 1 << 20).is_err());
        let _ = fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    #[test]
    fn keeps_permissions_and_symlink() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let d = tmpdir("perm");
        let real = d.join("real.txt");
        fs::write(&real, "old").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o640)).unwrap();
        let link = d.join("link.txt");
        symlink(&real, &link).unwrap();
        save(&link, b"new").unwrap();
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        assert_eq!(fs::metadata(&real).unwrap().permissions().mode() & 0o777, 0o640);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_file_is_new() {
        let l = load(Path::new("/definitely/not/here.txt"), 1 << 20).unwrap();
        assert!(!l.existed);
    }
}
