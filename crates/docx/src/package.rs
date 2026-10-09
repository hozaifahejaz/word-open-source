use crate::{
    DocxError, invalid,
    xml::{self, Node},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek, SeekFrom},
};

pub(crate) const MAX_PART: u64 = 8 * 1024 * 1024;
const MAX_TOTAL: u64 = 64 * 1024 * 1024;
pub(crate) struct Package {
    pub parts: BTreeMap<String, Vec<u8>>,
}
impl Package {
    pub fn read<R: Read + Seek>(mut input: R) -> Result<Self, DocxError> {
        let start = 0;
        let end = input.seek(SeekFrom::End(0))?;
        if end.saturating_sub(start) > MAX_TOTAL {
            return Err(DocxError::ResourceLimit(
                "compressed package exceeds 64 MiB".into(),
            ));
        }
        input.seek(SeekFrom::Start(start))?;
        let mut signature = [0; 8];
        let read = input.read(&mut signature)?;
        input.seek(SeekFrom::Start(start))?;
        if read == 8 && signature == [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1] {
            // Office encryption wraps the ZIP in an OLE compound file. Legacy DOC
            // also uses OLE: do not claim to have decoded either format.
            return Err(DocxError::UnsupportedPackage(
                "OLE compound file (encrypted Office or legacy .doc); decrypt/convert it".into(),
            ));
        }
        preflight(&mut input, end)?;
        input.seek(SeekFrom::Start(0))?;
        let mut archive = zip::ZipArchive::new(input)?;
        if archive.len() > 1024 {
            return Err(DocxError::ResourceLimit(
                "archive exceeds 1,024 entries".into(),
            ));
        }
        let mut parts = BTreeMap::new();
        let mut seen = BTreeSet::new();
        let mut total = 0u64;
        for i in 0..archive.len() {
            let mut part = archive.by_index(i).map_err(|e| match e {
                zip::result::ZipError::UnsupportedArchive(
                    zip::result::ZipError::PASSWORD_REQUIRED,
                ) => DocxError::EncryptedPackage,
                other => DocxError::Zip(other),
            })?;
            let name = part.name().to_string();
            if name.starts_with('/')
                || name.contains('\\')
                || name.split('/').any(|s| s == ".." || s == ".")
                || name.contains('\0')
            {
                return Err(invalid(format!("unsafe package part name {name:?}")));
            }
            if !seen.insert(name.clone()) {
                return Err(invalid(format!("duplicate package part {name}")));
            }
            if part.is_dir() {
                continue;
            }
            if part.size() > MAX_PART {
                return Err(DocxError::ResourceLimit(format!(
                    "part {name} exceeds 8 MiB"
                )));
            }
            total = total
                .checked_add(part.size())
                .ok_or_else(|| invalid("archive size overflow"))?;
            if total > MAX_TOTAL {
                return Err(DocxError::ResourceLimit(
                    "expanded archive exceeds 64 MiB".into(),
                ));
            }
            let mut data = Vec::new();
            (&mut part).take(MAX_PART + 1).read_to_end(&mut data)?;
            if data.len() as u64 > MAX_PART {
                return Err(DocxError::ResourceLimit(format!(
                    "part {name} exceeds 8 MiB"
                )));
            }
            parts.insert(name, data);
        }
        Ok(Self { parts })
    }
    pub fn xml(&self, name: &str) -> Result<Node, DocxError> {
        xml::parse(
            self.parts
                .get(name)
                .ok_or_else(|| invalid(format!("missing required part {name}")))?,
        )
        .map_err(|e| match e {
            DocxError::InvalidPackage(s) => invalid(format!("{name}: {s}")),
            other => other,
        })
    }
}
#[derive(Debug)]
pub(crate) struct Relationship {
    pub kind: String,
    pub target: String,
    pub external: bool,
}
pub(crate) fn relationships(node: &Node) -> Result<Vec<Relationship>, DocxError> {
    if node.ns != xml::REL || node.name != "Relationships" {
        return Err(invalid("invalid relationships root"));
    }
    let mut ids = BTreeSet::new();
    node.children
        .iter()
        .map(|n| {
            if n.ns != xml::REL || n.name != "Relationship" {
                return Err(invalid("invalid relationship element"));
            }
            let id = n
                .plain("Id")
                .ok_or_else(|| invalid("relationship missing Id"))?;
            if !ids.insert(id) {
                return Err(invalid("duplicate relationship Id"));
            }
            let mode = n.plain("TargetMode").unwrap_or("Internal");
            if !matches!(mode, "Internal" | "External") {
                return Err(invalid("invalid relationship TargetMode"));
            }
            Ok(Relationship {
                kind: n
                    .plain("Type")
                    .ok_or_else(|| invalid("relationship missing Type"))?
                    .into(),
                target: n
                    .plain("Target")
                    .ok_or_else(|| invalid("relationship missing Target"))?
                    .into(),
                external: mode == "External",
            })
        })
        .collect()
}
pub(crate) fn target(base: &str, path: &str) -> Result<String, DocxError> {
    if path.contains(['\\', '?', '#', '%', ':', '\0']) {
        return Err(DocxError::UnsupportedPackage(format!(
            "unsupported relationship URI {path:?}"
        )));
    }
    let mut components: Vec<&str> = if path.starts_with('/') {
        Vec::new()
    } else {
        base.rsplit_once('/')
            .map(|(dir, _)| dir.split('/').collect())
            .unwrap_or_default()
    };
    for c in path.trim_start_matches('/').split('/') {
        match c {
            "" | "." => {}
            ".." => {
                if components.pop().is_none() {
                    return Err(invalid("relationship escapes package root"));
                }
            }
            _ => components.push(c),
        }
    }
    if components.is_empty() {
        return Err(invalid("empty relationship target"));
    }
    Ok(components.join("/"))
}
pub(crate) fn rels_path(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

// Bound the central directory before zip allocates its entry table. ZIP64 and
// split/SFX archives are outside this deliberately small package subset.
fn preflight<R: Read + Seek>(input: &mut R, end: u64) -> Result<(), DocxError> {
    let tail_len = end.min(65_557) as usize;
    input.seek(SeekFrom::Start(end - tail_len as u64))?;
    let mut tail = vec![0; tail_len];
    input.read_exact(&mut tail)?;
    let pos = (0..tail.len().saturating_sub(21))
        .rev()
        .find(|&i| {
            tail[i..].starts_with(b"PK\x05\x06")
                && i + 22 + usize::from(u16::from_le_bytes([tail[i + 20], tail[i + 21]]))
                    == tail.len()
        })
        .ok_or_else(|| invalid("ZIP end-of-central-directory missing or truncated"))?;
    let e = &tail[pos..];
    let short = |i: usize| u16::from_le_bytes([e[i], e[i + 1]]);
    let long = |i: usize| u32::from_le_bytes([e[i], e[i + 1], e[i + 2], e[i + 3]]);
    if short(4) != 0 || short(6) != 0 || short(8) != short(10) {
        return Err(DocxError::UnsupportedPackage("multi-disk ZIP".into()));
    }
    if short(10) == u16::MAX
        || long(12) == u32::MAX
        || long(16) == u32::MAX
        || (pos >= 20 && tail[pos - 20..].starts_with(b"PK\x06\x07"))
    {
        return Err(DocxError::UnsupportedPackage(
            "ZIP64 is unnecessary for the bounded DOCX subset".into(),
        ));
    }
    if short(10) > 1024 || long(12) > 1024 * 1024 {
        return Err(DocxError::ResourceLimit(
            "ZIP central directory exceeds 1,024 entries or 1 MiB".into(),
        ));
    }
    let cd_end = u64::from(long(16)) + u64::from(long(12));
    if cd_end != end - tail_len as u64 + pos as u64 {
        return Err(invalid(
            "ZIP central directory offset/size mismatch (embedded/SFX ZIP unsupported)",
        ));
    }
    input.seek(SeekFrom::Start(u64::from(long(16))))?;
    let mut cd = vec![0; long(12) as usize];
    input.read_exact(&mut cd)?;
    let mut offset = 0usize;
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for _ in 0..short(10) {
        let h = cd
            .get(offset..offset + 46)
            .ok_or_else(|| invalid("truncated ZIP central header"))?;
        if !h.starts_with(b"PK\x01\x02") {
            return Err(invalid("invalid ZIP central header"));
        }
        let u16_at = |i: usize| u16::from_le_bytes([h[i], h[i + 1]]);
        let u32_at = |i: usize| u32::from_le_bytes([h[i], h[i + 1], h[i + 2], h[i + 3]]);
        if u16_at(8) & 0x41 != 0 {
            return Err(DocxError::EncryptedPackage);
        }
        if !matches!(u16_at(10), 0 | 8) {
            return Err(DocxError::UnsupportedPackage(
                "ZIP compression must be Stored or Deflate".into(),
            ));
        }
        if u16_at(34) != 0 {
            return Err(DocxError::UnsupportedPackage("multi-disk ZIP entry".into()));
        }
        if u64::from(u32_at(24)) > MAX_PART {
            return Err(DocxError::ResourceLimit(
                "archive part exceeds 8 MiB".into(),
            ));
        }
        total += u64::from(u32_at(24));
        if total > MAX_TOTAL {
            return Err(DocxError::ResourceLimit(
                "expanded archive exceeds 64 MiB".into(),
            ));
        }
        let name_len = usize::from(u16_at(28));
        if name_len > 4096 {
            return Err(DocxError::ResourceLimit(
                "package part name exceeds 4,096 bytes".into(),
            ));
        }
        let name = cd
            .get(offset + 46..offset + 46 + name_len)
            .ok_or_else(|| invalid("truncated ZIP part name"))?;
        let name =
            std::str::from_utf8(name).map_err(|_| invalid("package part names must be UTF-8"))?;
        if !names.insert(name) {
            return Err(invalid(format!("duplicate package part {name}")));
        }
        offset += 46 + name_len + usize::from(u16_at(30)) + usize::from(u16_at(32));
        if offset > cd.len() {
            return Err(invalid("truncated ZIP central directory"));
        }
    }
    if offset != cd.len() {
        return Err(invalid(
            "ZIP central directory contains unexpected trailing data",
        ));
    }
    Ok(())
}
