use crate::{DocxError, invalid};
use quick_xml::{Reader, events::Event};
use std::collections::BTreeMap;

pub(crate) const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub(crate) const STRICT_W: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";
pub(crate) const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
pub(crate) const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
#[derive(Debug, Default)]
pub(crate) struct Node {
    pub ns: String,
    pub name: String,
    pub attrs: BTreeMap<(String, String), String>,
    pub children: Vec<Node>,
    pub text: String,
}
impl Node {
    pub fn is(&self, name: &str) -> bool {
        (self.ns == W || self.ns == STRICT_W) && self.name == name
    }
    pub fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|n| n.is(name))
    }
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .get(&(W.into(), name.into()))
            .or_else(|| self.attrs.get(&(STRICT_W.into(), name.into())))
            .map(String::as_str)
    }
    pub fn plain(&self, name: &str) -> Option<&str> {
        self.attrs
            .get(&(String::new(), name.into()))
            .map(String::as_str)
    }
}
fn qualified(
    raw: &[u8],
    namespaces: &BTreeMap<String, String>,
    attribute: bool,
) -> Result<(String, String), DocxError> {
    let raw = std::str::from_utf8(raw).map_err(|_| invalid("XML names must be UTF-8"))?;
    if raw.len() > 256 {
        return Err(DocxError::ResourceLimit(
            "XML name exceeds 256 bytes".into(),
        ));
    }
    if let Some((prefix, local)) = raw.split_once(':') {
        if local.contains(':') || local.is_empty() {
            return Err(invalid("invalid qualified XML name"));
        }
        Ok((
            namespaces
                .get(prefix)
                .ok_or_else(|| invalid(format!("unbound XML prefix {prefix}")))?
                .clone(),
            local.into(),
        ))
    } else {
        Ok((
            if attribute {
                String::new()
            } else {
                namespaces.get("").cloned().unwrap_or_default()
            },
            raw.into(),
        ))
    }
}
pub(crate) fn parse(bytes: &[u8]) -> Result<Node, DocxError> {
    std::str::from_utf8(bytes)
        .map_err(|_| invalid("XML must use UTF-8; convert the document in Word first"))?;
    if std::str::from_utf8(bytes).expect("checked UTF-8").chars().any(|c| !matches!(c as u32, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)) {
        return Err(invalid("XML contains forbidden XML 1.0 control characters; remove them or resave an unencrypted .docx in Word"));
    }
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().check_end_names = true;
    let mut stack: Vec<(Node, BTreeMap<String, String>)> = Vec::new();
    let mut root = None;
    let mut count = 0;
    loop {
        count += 1;
        if count > 200_000 {
            return Err(DocxError::ResourceLimit(
                "XML exceeds 200,000 events".into(),
            ));
        }
        let event = reader.read_event()?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                if stack.len() >= 64 {
                    return Err(DocxError::ResourceLimit("XML nesting exceeds 64".into()));
                }
                let mut ns = stack.last().map(|s| s.1.clone()).unwrap_or_else(|| {
                    BTreeMap::from([("xml".into(), "http://www.w3.org/XML/1998/namespace".into())])
                });
                let mut attrs = Vec::new();
                for (attribute_count, a) in e.attributes().enumerate() {
                    if attribute_count >= 128 {
                        return Err(DocxError::ResourceLimit(
                            "XML element exceeds 128 attributes".into(),
                        ));
                    }
                    let a = a.map_err(|err| invalid(format!("invalid XML attribute: {err}")))?;
                    let key = std::str::from_utf8(a.key.as_ref())
                        .map_err(|_| invalid("invalid XML attribute name"))?
                        .to_string();
                    let val = a.decode_and_unescape_value(reader.decoder()).map_err(|e| invalid(format!("invalid XML attribute: {e}; remove invalid character references or resave an unencrypted .docx in Word")))?.into_owned();
                    valid_chars(&val)?;
                    if key.len() > 256 || val.len() > 65_536 {
                        return Err(DocxError::ResourceLimit(
                            "XML attribute name/value exceeds 256 bytes/64 KiB".into(),
                        ));
                    }
                    if (key == "xmlns" || key.starts_with("xmlns:")) && val.len() > 256 {
                        return Err(DocxError::ResourceLimit(
                            "XML namespace URI exceeds 256 bytes".into(),
                        ));
                    }
                    if key == "xmlns" {
                        ns.insert(String::new(), val);
                    } else if let Some(p) = key.strip_prefix("xmlns:") {
                        ns.insert(p.into(), val);
                    } else {
                        attrs.push((key, val));
                    }
                }
                if ns.len() > 128 {
                    return Err(DocxError::ResourceLimit(
                        "XML namespace context exceeds 128 bindings".into(),
                    ));
                }
                let (namespace, name) = qualified(e.name().as_ref(), &ns, false)?;
                let mut node = Node {
                    ns: namespace,
                    name,
                    ..Node::default()
                };
                for (key, val) in attrs {
                    if node
                        .attrs
                        .insert(qualified(key.as_bytes(), &ns, true)?, val)
                        .is_some()
                    {
                        return Err(invalid("duplicate XML attribute"));
                    }
                }
                if empty {
                    attach(node, &mut stack, &mut root)?;
                } else {
                    stack.push((node, ns));
                }
            }
            Event::End(_) => {
                let (node, _) = stack
                    .pop()
                    .ok_or_else(|| invalid("unexpected XML closing tag"))?;
                attach(node, &mut stack, &mut root)?;
            }
            Event::Text(t) => {
                let value = t.xml_content().map_err(|e| invalid(e.to_string()))?;
                if let Some((node, _)) = stack.last_mut() {
                    node.text.push_str(&value);
                } else if !value.trim().is_empty() {
                    return Err(invalid("text outside XML root"));
                }
            }
            Event::GeneralRef(r) => {
                let name = r.decode().map_err(|e| invalid(e.to_string()))?;
                let resolved = quick_xml::escape::unescape(&format!("&{name};"))
                    .map_err(|e| invalid(format!("invalid XML entity: {e}; remove invalid character references or resave an unencrypted .docx in Word")))?
                    .into_owned();
                valid_chars(&resolved)?;
                stack
                    .last_mut()
                    .ok_or_else(|| invalid("entity outside root"))?
                    .0
                    .text
                    .push_str(&resolved);
            }
            Event::CData(t) => {
                stack
                    .last_mut()
                    .ok_or_else(|| invalid("CDATA outside root"))?
                    .0
                    .text
                    .push_str(&t.decode().map_err(|e| invalid(e.to_string()))?);
            }
            Event::DocType(_) => {
                return Err(invalid(
                    "DTD/entity declarations are unsupported; remove them before opening",
                ));
            }
            Event::Decl(d) => {
                if let Some(enc) = d.encoding() {
                    let enc = enc.map_err(|e| invalid(e.to_string()))?;
                    if !enc.eq_ignore_ascii_case(b"utf-8") {
                        return Err(invalid("only UTF-8 XML is supported"));
                    }
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::PI(_) => {}
        }
    }
    if !stack.is_empty() {
        return Err(invalid("unclosed XML element"));
    }
    root.ok_or_else(|| invalid("empty XML part"))
}
fn attach(
    node: Node,
    stack: &mut [(Node, BTreeMap<String, String>)],
    root: &mut Option<Node>,
) -> Result<(), DocxError> {
    if let Some((parent, _)) = stack.last_mut() {
        parent.children.push(node);
    } else if root.replace(node).is_some() {
        return Err(invalid("multiple XML roots"));
    }
    Ok(())
}

fn valid_chars(value: &str) -> Result<(), DocxError> {
    if value.chars().any(|c| !matches!(c as u32, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)) {
        return Err(invalid("XML entity or attribute contains forbidden XML 1.0 control characters; remove them or resave an unencrypted .docx in Word"));
    }
    Ok(())
}
