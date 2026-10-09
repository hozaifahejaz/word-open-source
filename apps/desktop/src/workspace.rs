//! Private, versioned local workspace and recovery persistence.
use document_core::{Document, ImportWarning, Position, Selection};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    env,
    error::Error,
    fmt, fs, io,
    path::{Path, PathBuf},
};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    MalformedJson(serde_json::Error),
    UnsupportedSchemaVersion(u32),
    InvalidPath(String),
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Workspace IO error: {e}"),
            Self::MalformedJson(e) => write!(f, "Malformed workspace JSON: {e}"),
            Self::UnsupportedSchemaVersion(v) => {
                write!(f, "Unsupported workspace schema version: {v}")
            }
            Self::InvalidPath(e) => write!(f, "Invalid stored path: {e}"),
        }
    }
}
impl Error for StoreError {}
impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        Self::MalformedJson(e)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "encoding", content = "bytes")]
pub enum StoredPath {
    #[serde(rename = "unix-bytes")]
    UnixBytes(Vec<u8>),
    #[serde(rename = "windows-utf16le")]
    WindowsUtf16Le(Vec<u8>),
}
impl StoredPath {
    pub fn from_path(path: &Path) -> Result<Self, StoreError> {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            Ok(Self::UnixBytes(path.as_os_str().as_bytes().to_vec()))
        }
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            Ok(Self::WindowsUtf16Le(
                path.as_os_str()
                    .encode_wide()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            ))
        }
    }
    pub fn to_path(&self) -> Result<PathBuf, StoreError> {
        #[cfg(unix)]
        {
            use std::{ffi::OsString, os::unix::ffi::OsStringExt};
            match self {
                Self::UnixBytes(bytes) => Ok(PathBuf::from(OsString::from_vec(bytes.clone()))),
                _ => Err(StoreError::InvalidPath(
                    "path encoding does not match this operating system".into(),
                )),
            }
        }
        #[cfg(windows)]
        {
            use std::{ffi::OsString, os::windows::ffi::OsStringExt};
            match self {
                Self::WindowsUtf16Le(bytes) if bytes.len() % 2 == 0 => {
                    let units: Vec<_> = bytes
                        .chunks_exact(2)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]))
                        .collect();
                    Ok(PathBuf::from(OsString::from_wide(&units)))
                }
                _ => Err(StoreError::InvalidPath(
                    "expected an even-length Windows UTF-16LE path".into(),
                )),
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecentDocument {
    pub path: StoredPath,
    pub last_opened_unix_seconds: u64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedCaret {
    pub path: StoredPath,
    pub position: Position,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub dark_mode: bool,
    pub zoom: f32,
    pub recent: Vec<RecentDocument>,
    pub carets: Vec<SavedCaret>,
}
impl Default for WorkspaceState {
    fn default() -> Self {
        Self {
            dark_mode: false,
            zoom: 1.0,
            recent: vec![],
            carets: vec![],
        }
    }
}
impl WorkspaceState {
    pub fn record_recent(&mut self, path: StoredPath, timestamp: u64) {
        self.remove_recent(&path);
        self.recent.insert(
            0,
            RecentDocument {
                path,
                last_opened_unix_seconds: timestamp,
            },
        );
        self.recent.truncate(12);
    }
    pub fn remove_recent(&mut self, path: &StoredPath) {
        self.recent.retain(|r| &r.path != path);
    }
    pub fn set_caret(&mut self, path: StoredPath, position: Position) {
        self.carets.retain(|c| c.path != path);
        self.carets.push(SavedCaret { path, position });
    }
    pub fn caret_for(&self, path: &StoredPath) -> Option<Position> {
        self.carets
            .iter()
            .find(|c| &c.path == path)
            .map(|c| c.position)
    }
    fn normalize(&mut self) {
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(0.25, 2.5)
        } else {
            1.0
        };
        let mut seen = Vec::new();
        self.recent.retain(|r| {
            if seen.contains(&r.path) {
                false
            } else {
                seen.push(r.path.clone());
                true
            }
        });
        self.recent.truncate(12);
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecoverySnapshot {
    pub document: Document,
    pub path: Option<StoredPath>,
    pub protected_source: Option<StoredPath>,
    pub warnings: Vec<ImportWarning>,
    pub selection: Selection,
    pub captured_unix_seconds: u64,
}
#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    schema_version: u32,
    data: T,
}
#[derive(Deserialize)]
struct Header {
    schema_version: u32,
}

pub struct WorkspaceStore {
    root: PathBuf,
}
impl WorkspaceStore {
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn for_user() -> Result<Self, StoreError> {
        fn required(name: &str) -> Result<PathBuf, StoreError> {
            env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .ok_or_else(|| {
                    StoreError::Io(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("{name} is unavailable"),
                    ))
                })
        }
        #[cfg(windows)]
        let root = required("APPDATA")?.join("Folio");
        #[cfg(target_os = "macos")]
        let root = required("HOME")?.join("Library/Application Support/Folio");
        #[cfg(all(unix, not(target_os = "macos")))]
        let root = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .map(|p| Ok(p.join("folio")))
            .unwrap_or_else(|| required("HOME").map(|p| p.join(".local/share/folio")))?;
        Ok(Self::at(root))
    }
    pub fn load_state(&self) -> Result<Option<WorkspaceState>, StoreError> {
        let mut state: Option<WorkspaceState> = self.load("workspace.json")?;
        if let Some(state) = &mut state {
            state.normalize();
        }
        Ok(state)
    }
    pub fn save_state(&self, state: &WorkspaceState) -> Result<(), StoreError> {
        let mut state = state.clone();
        state.normalize();
        self.save("workspace.json", &state)
    }
    pub fn load_recovery(&self) -> Result<Option<RecoverySnapshot>, StoreError> {
        self.load("recovery.json")
    }
    pub fn save_recovery(&self, snapshot: &RecoverySnapshot) -> Result<(), StoreError> {
        self.save("recovery.json", snapshot)
    }
    pub fn clear_recovery(&self) -> Result<(), StoreError> {
        match fs::remove_file(self.root.join("recovery.json")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
    fn load<T: DeserializeOwned>(&self, name: &str) -> Result<Option<T>, StoreError> {
        let bytes = match fs::read(self.root.join(name)) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let header: Header = serde_json::from_slice(&bytes)?;
        if header.schema_version != SCHEMA_VERSION {
            return Err(StoreError::UnsupportedSchemaVersion(header.schema_version));
        }
        let envelope: Envelope<T> = serde_json::from_slice(&bytes)?;
        Ok(Some(envelope.data))
    }
    fn save<T: Serialize + DeserializeOwned>(
        &self,
        name: &str,
        data: &T,
    ) -> Result<(), StoreError> {
        // Refuse to replace unreadable or unsupported state, even with defaults.
        let _: Option<T> = self.load(name)?;
        let bytes = serde_json::to_vec_pretty(&Envelope {
            schema_version: SCHEMA_VERSION,
            data,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&self.root)?;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        #[cfg(not(unix))]
        fs::create_dir_all(&self.root)?;
        crate::files::atomic_write(&self.root.join(name), &bytes)
            .map_err(|e| StoreError::Io(io::Error::other(e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "folio-workspace-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn path(name: &str) -> StoredPath {
        StoredPath::from_path(Path::new(name)).unwrap()
    }
    #[test]
    fn workspace_state_round_trips_preferences_and_recent_order() {
        let dir = Directory::new();
        let store = WorkspaceStore::at(dir.0.clone());
        assert!(store.load_state().unwrap().is_none());
        let mut state = WorkspaceState {
            dark_mode: true,
            zoom: 1.75,
            ..Default::default()
        };
        state.record_recent(path("missing-a.docx"), 10);
        state.record_recent(path("missing-b.docx"), 20);
        state.set_caret(path("missing-a.docx"), Position::new(3, 7));
        store.save_state(&state).unwrap();
        let loaded = store.load_state().unwrap().unwrap();
        assert!(loaded.dark_mode);
        assert_eq!(loaded.zoom, 1.75);
        assert_eq!(
            loaded
                .recent
                .iter()
                .map(|r| r.path.to_path().unwrap())
                .collect::<Vec<_>>(),
            vec![
                PathBuf::from("missing-b.docx"),
                PathBuf::from("missing-a.docx")
            ]
        );
        assert_eq!(loaded.recent[0].last_opened_unix_seconds, 20);
        assert_eq!(
            loaded.caret_for(&path("missing-a.docx")),
            Some(Position::new(3, 7))
        );
    }
    #[test]
    fn recent_list_is_unique_and_limited_to_twelve() {
        let mut state = WorkspaceState::default();
        for n in 0..14 {
            state.record_recent(path(&format!("{n}.docx")), n);
        }
        state.record_recent(path("5.docx"), 30);
        assert_eq!(state.recent.len(), 12);
        assert_eq!(state.recent[0].path, path("5.docx"));
        assert_eq!(state.recent[0].last_opened_unix_seconds, 30);
        assert_eq!(state.recent.last().unwrap().path, path("2.docx"));
        assert_eq!(
            state
                .recent
                .iter()
                .filter(|r| r.path == path("5.docx"))
                .count(),
            1
        );
        state.remove_recent(&path("5.docx"));
        assert_eq!(state.recent.len(), 11);
        state.set_caret(path("2.docx"), Position::new(1, 2));
        state.set_caret(path("2.docx"), Position::new(2, 3));
        assert_eq!(state.carets.len(), 1);
        assert_eq!(state.caret_for(&path("2.docx")), Some(Position::new(2, 3)));
    }
    #[cfg(unix)]
    #[test]
    fn unix_non_utf8_path_round_trips() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};
        let original = PathBuf::from(OsString::from_vec(b"/missing/\xff.docx".to_vec()));
        let stored = StoredPath::from_path(&original).unwrap();
        let json = serde_json::to_vec(&stored).unwrap();
        assert_eq!(
            serde_json::from_slice::<StoredPath>(&json)
                .unwrap()
                .to_path()
                .unwrap(),
            original
        );
    }
    #[cfg(windows)]
    #[test]
    fn windows_utf16_path_round_trips() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};
        let original = PathBuf::from(OsString::from_wide(&[67, 58, 92, 0xd800, 46, 100]));
        let stored = StoredPath::from_path(&original).unwrap();
        let json = serde_json::to_vec(&stored).unwrap();
        assert_eq!(
            serde_json::from_slice::<StoredPath>(&json)
                .unwrap()
                .to_path()
                .unwrap(),
            original
        );
    }
    #[test]
    fn future_schema_is_rejected_without_changing_bytes() {
        let dir = Directory::new();
        fs::create_dir_all(&dir.0).unwrap();
        let bytes = br#"{"schema_version":999,"future":"data"}"#;
        fs::write(dir.0.join("workspace.json"), bytes).unwrap();
        let store = WorkspaceStore::at(dir.0.clone());
        assert!(matches!(
            store.load_state(),
            Err(StoreError::UnsupportedSchemaVersion(999))
        ));
        assert!(store.save_state(&WorkspaceState::default()).is_err());
        assert_eq!(fs::read(dir.0.join("workspace.json")).unwrap(), bytes);
    }
    #[test]
    fn malformed_state_is_reported_without_replacement() {
        let dir = Directory::new();
        fs::create_dir_all(&dir.0).unwrap();
        fs::write(dir.0.join("workspace.json"), b"{broken").unwrap();
        let store = WorkspaceStore::at(dir.0.clone());
        assert!(matches!(
            store.load_state(),
            Err(StoreError::MalformedJson(_))
        ));
        assert!(store.save_state(&WorkspaceState::default()).is_err());
        assert_eq!(fs::read(dir.0.join("workspace.json")).unwrap(), b"{broken");
    }
    #[test]
    fn failed_state_write_preserves_previous_file() {
        let dir = Directory::new();
        fs::create_dir_all(dir.0.join("workspace.json")).unwrap();
        let previous = dir.0.join("workspace.json/previous");
        fs::write(&previous, b"previous").unwrap();
        let store = WorkspaceStore::at(dir.0.clone());
        assert!(matches!(
            store.save_state(&WorkspaceState::default()),
            Err(StoreError::Io(_))
        ));
        assert_eq!(fs::read(previous).unwrap(), b"previous");
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
    #[test]
    fn recovery_round_trips_and_clears() {
        let dir = Directory::new();
        let store = WorkspaceStore::at(dir.0.clone());
        assert!(store.load_recovery().unwrap().is_none());
        store.clear_recovery().unwrap();
        let snapshot = RecoverySnapshot {
            document: Document::default(),
            path: Some(path("draft.docx")),
            protected_source: Some(path("original.docx")),
            warnings: vec![],
            selection: Selection::caret(Position::new(0, 0)),
            captured_unix_seconds: 42,
        };
        store.save_recovery(&snapshot).unwrap();
        let loaded = store.load_recovery().unwrap().unwrap();
        assert_eq!(loaded, snapshot);
        store.clear_recovery().unwrap();
        assert!(store.load_recovery().unwrap().is_none());
    }
    #[test]
    fn zoom_is_clamped_and_nonfinite_uses_default() {
        let dir = Directory::new();
        let store = WorkspaceStore::at(dir.0.clone());
        for (zoom, expected) in [(0.0, 0.25), (10.0, 2.5), (f32::NAN, 1.0)] {
            store
                .save_state(&WorkspaceState {
                    zoom,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(store.load_state().unwrap().unwrap().zoom, expected);
        }
    }
    #[cfg(unix)]
    #[test]
    fn store_directory_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = Directory::new();
        let store = WorkspaceStore::at(dir.0.clone());
        store.save_state(&WorkspaceState::default()).unwrap();
        assert_eq!(
            fs::metadata(&dir.0).unwrap().permissions().mode() & 0o777,
            0o700
        );
        fs::set_permissions(&dir.0, fs::Permissions::from_mode(0o755)).unwrap();
        store.save_state(&WorkspaceState::default()).unwrap();
        assert_eq!(
            fs::metadata(&dir.0).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}
