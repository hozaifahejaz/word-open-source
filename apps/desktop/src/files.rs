//! Caller-owned codec streams and failure-safe filesystem replacement.
use document_core::{Block, Document, ImportReport, Selection};
use std::{
    fs::{self, File, OpenOptions},
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub fn open(path: &Path) -> Result<ImportReport, String> {
    folio_docx::import_docx(File::open(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
pub fn same_file(a: &Path, b: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (fs::canonicalize(a), fs::canonicalize(b))
        && a == b
    {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(a), Ok(b)) = (fs::metadata(a), fs::metadata(b)) {
            return a.dev() == b.dev() && a.ino() == b.ino();
        }
    }
    #[cfg(windows)]
    {
        if let (Ok(a), Ok(b)) = (File::open(a), File::open(b)) {
            return windows::identity(&a)
                .zip(windows::identity(&b))
                .is_some_and(|(a, b)| a == b);
        }
    }
    a == b
}
pub fn save(doc: &Document, path: &Path, protected: Option<&Path>) -> Result<(), String> {
    if protected.is_some_and(|source| same_file(source, path)) {
        return Err("This import contains warnings. Save a converted copy to a different file; the source cannot be overwritten.".into());
    }
    // Encode and check warnings before touching any destination.
    let mut bytes = Cursor::new(Vec::new());
    let report = folio_docx::export_docx(doc, &mut bytes).map_err(|e| e.to_string())?;
    if !report.warnings.is_empty() {
        return Err(format!(
            "Export reported unsupported content; save was stopped: {}",
            report
                .warnings
                .iter()
                .map(|w| w.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    atomic_write(path, bytes.get_ref())
}
/// All copy/export adapters share identity and explicit overwrite decisions.
pub fn check_destination(
    path: &Path,
    extension: &str,
    source: Option<&Path>,
    overwrite: bool,
) -> Result<(), String> {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
    {
        return Err(format!("Choose an explicit .{extension} destination"));
    }
    if source.is_some_and(|source| same_file(source, path)) {
        return Err("Choose a different file; the active source and its aliases cannot be replaced by this operation".into());
    }
    if path.exists() && !overwrite {
        return Err("The destination exists. Confirm replacement first".into());
    }
    Ok(())
}
pub fn plain_text(doc: &Document, selection: Option<Selection>) -> Result<String, String> {
    doc.validate().map_err(|e| e.to_string())?;
    let selection = selection.unwrap_or_else(|| crate::editing::select_all(doc));
    doc.validate_selection(selection)
        .map_err(|e| e.to_string())?;
    let (start, end) = selection.ordered();
    let mut text = String::new();
    for i in start.block..=end.block {
        if i != start.block {
            text.push('\n');
        }
        match &doc.blocks[i] {
            Block::PageBreak => text.push('\u{000c}'),
            Block::Paragraph(p) => {
                let value = p.text();
                let from = if i == start.block { start.offset } else { 0 };
                let to = if i == end.block {
                    end.offset
                } else {
                    value.len()
                };
                text.push_str(&value[from..to]);
            }
        }
    }
    Ok(text)
}
pub fn export_text(
    doc: &Document,
    path: &Path,
    selection: Option<Selection>,
    source: Option<&Path>,
    overwrite: bool,
) -> Result<(), String> {
    check_destination(path, "txt", source, overwrite)?;
    atomic_write(path, plain_text(doc, selection)?.as_bytes())
}
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or("Choose a file name")?
        .to_string_lossy();
    let (temporary, mut file) = loop {
        let p = parent.join(format!(
            ".{name}.folio-{}-{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(f) => break (Temporary(p), f),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    file.write_all(bytes)
        .and_then(|_| file.flush())
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    // Same-directory rename is atomic; never remove the old destination first.
    #[cfg(not(windows))]
    fs::rename(&temporary.0, path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    windows::replace(&temporary.0, path).map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(windows)]
mod windows {
    use std::{
        ffi::c_void,
        fs::File,
        io,
        os::windows::{ffi::OsStrExt, io::AsRawHandle},
        path::Path,
    };
    #[repr(C)]
    #[derive(Default)]
    struct Info {
        attributes: u32,
        created: [u32; 2],
        accessed: [u32; 2],
        written: [u32; 2],
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
        fn GetFileInformationByHandle(handle: *mut c_void, info: *mut Info) -> i32;
    }
    pub fn identity(f: &File) -> Option<(u32, u32, u32)> {
        let mut i = Info::default();
        if unsafe { GetFileInformationByHandle(f.as_raw_handle(), &mut i) } == 0 {
            None
        } else {
            Some((i.volume, i.index_high, i.index_low))
        }
    }
    pub fn replace(a: &Path, b: &Path) -> io::Result<()> {
        let a: Vec<u16> = a.as_os_str().encode_wide().chain(Some(0)).collect();
        let b: Vec<u16> = b.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 1 | 8) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "folio-files-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn text_export_preserves_breaks_reversed_selection_and_requires_overwrite() {
        use document_core::{Block, Paragraph, Position, Selection};
        let doc = Document {
            blocks: vec![
                Block::Paragraph(Paragraph::plain("e\u{301} hello")),
                Block::PageBreak,
                Block::Paragraph(Paragraph::plain("world")),
            ],
            ..Default::default()
        };
        assert_eq!(
            plain_text(&doc, None).unwrap(),
            "e\u{301} hello\n\u{000c}\nworld"
        );
        let selection = Selection::new(Position::new(2, 3), Position::new(0, 3));
        assert_eq!(
            plain_text(&doc, Some(selection)).unwrap(),
            " hello\n\u{000c}\nwor"
        );
        assert!(plain_text(&doc, Some(Selection::caret(Position::new(0, 1)))).is_err());
        let dir = directory();
        let path = dir.join("out.txt");
        fs::write(&path, "original").unwrap();
        assert!(export_text(&doc, &path, None, None, false).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
        export_text(&doc, &path, None, None, true).unwrap();
        let alias = dir.join("alias.txt");
        fs::hard_link(&path, &alias).unwrap();
        assert!(export_text(&doc, &alias, None, Some(&path), true).is_err());
        assert!(export_text(&doc, &dir.join("out.docx"), None, None, false).is_err());
        assert!(export_text(&doc, &dir.join("missing/out.txt"), None, None, false).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn atomic_replacement_and_warning_alias_guard() {
        let dir = directory();
        let file = dir.join("original.docx");
        fs::write(&file, b"original").unwrap();
        atomic_write(&file, b"new").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"new");
        let alias = dir.join("alias.docx");
        fs::hard_link(&file, &alias).unwrap();
        assert!(same_file(&file, &alias));
        assert!(save(&Document::default(), &alias, Some(&file)).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"new");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn failed_replace_keeps_destination_and_cleans_temp() {
        let dir = directory();
        let dest = dir.join("directory.docx");
        fs::create_dir(&dest).unwrap();
        assert!(atomic_write(&dest, b"test").is_err());
        assert!(dest.is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
