//! Bounded, stream-based DOCX conversion for the milestone subset.
//!
//! Import warnings indicate fidelity loss. Callers must retain the original source
//! and require Save As to a different, non-aliasing path; export writes only the
//! model, never unknown package content. See the crate README for limits/sources.
mod codec;
mod formatting;
mod package;
mod xml;
use document_core::{CoreError, Document, ImportReport, ImportWarning};
use std::{
    error::Error,
    fmt,
    io::{self, Read, Seek, Write},
};

#[derive(Debug)]
pub enum DocxError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    Xml(quick_xml::Error),
    InvalidDocument(CoreError),
    NotImplemented,
    EncryptedPackage,
    UnsupportedPackage(String),
    InvalidPackage(String),
    ResourceLimit(String),
}
impl fmt::Display for DocxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "DOCX I/O: {e}"),
            Self::Zip(e) => write!(f, "DOCX archive: {e}"),
            Self::Xml(e) => write!(f, "DOCX XML: {e}"),
            Self::InvalidDocument(e) => write!(f, "invalid document: {e}"),
            Self::EncryptedPackage => {
                f.write_str("encrypted DOCX: decrypt and save an unencrypted .docx in Word first")
            }
            Self::UnsupportedPackage(s) => write!(
                f,
                "unsupported DOCX package: {s}; save a standard .docx in Word first"
            ),
            Self::NotImplemented => f.write_str("DOCX codec is a downstream implementation stub"),
            Self::InvalidPackage(s) => write!(f, "invalid DOCX package: {s}"),
            Self::ResourceLimit(s) => write!(f, "DOCX resource limit: {s}"),
        }
    }
}
impl Error for DocxError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Zip(e) => Some(e),
            Self::Xml(e) => Some(e),
            Self::InvalidDocument(e) => Some(e),
            _ => None,
        }
    }
}
impl From<io::Error> for DocxError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<zip::result::ZipError> for DocxError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::Zip(e)
    }
}
impl From<quick_xml::Error> for DocxError {
    fn from(e: quick_xml::Error) -> Self {
        Self::Xml(e)
    }
}
impl From<CoreError> for DocxError {
    fn from(e: CoreError) -> Self {
        Self::InvalidDocument(e)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportReport {
    pub warnings: Vec<ImportWarning>,
}

/// Converts supported content; any warning requires a converted-copy save.
pub fn import_docx<R: Read + Seek>(reader: R) -> Result<ImportReport, DocxError> {
    codec::import(reader)
}
/// Writes a new package containing only the supported document model.
pub fn export_docx<W: Write + Seek>(
    document: &Document,
    writer: W,
) -> Result<ExportReport, DocxError> {
    codec::export(document, writer)
}
pub fn requires_converted_copy(report: &ImportReport) -> bool {
    !report.warnings.is_empty()
}
pub(crate) fn invalid(message: impl Into<String>) -> DocxError {
    DocxError::InvalidPackage(message.into())
}
