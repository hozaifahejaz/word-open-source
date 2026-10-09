use crate::{
    DocxError, invalid,
    xml::{Node, STRICT_W, W},
};
use document_core::*;
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Diagnostics {
    pub warnings: Vec<ImportWarning>,
    pub part: String,
}
impl Diagnostics {
    pub fn warn(&mut self, feature: Feature, message: impl Into<String>) {
        self.record(WarningCode::UnsupportedFeature, feature, message.into());
    }
    pub fn approximate(&mut self, feature: Feature, message: impl Into<String>) {
        self.record(WarningCode::ApproximatedFormatting, feature, message.into());
    }
    pub fn missing_style(&mut self, message: impl Into<String>) {
        self.record(WarningCode::MissingPart, Feature::Styles, message.into());
    }
    fn record(&mut self, code: WarningCode, feature: Feature, message: String) {
        if self.warnings.len() >= 512 {
            return;
        }
        let warning = ImportWarning {
            code,
            feature,
            location: Some(self.part.clone()),
            message,
        };
        if !self.warnings.contains(&warning) {
            if self.warnings.len() < 511 {
                self.warnings.push(warning);
            } else if self.warnings.len() == 511 {
                self.warnings.push(ImportWarning {
                    code: WarningCode::ResourceLimit,
                    feature: Feature::Other("diagnostics".into()),
                    location: None,
                    message:
                        "Additional unsupported content diagnostics omitted; save a converted copy"
                            .into(),
                });
            }
        }
    }
    pub fn unknown(&mut self, n: &Node) {
        let feature = match n.name.as_str() {
            "tbl" => Feature::Tables,
            "drawing" | "pict" => Feature::Images,
            "numPr" => Feature::Lists,
            "sectPr" | "cols" => Feature::Sections,
            "headerReference" | "footerReference" => Feature::HeadersFooters,
            "fldSimple" | "fldChar" | "instrText" => Feature::Fields,
            "hyperlink" | "footnoteReference" | "endnoteReference" => Feature::References,
            "ins" | "del" | "commentRangeStart" | "commentRangeEnd" | "commentReference" => {
                Feature::Reviewing
            }
            "object" | "altChunk" => Feature::EmbeddedObjects,
            _ => Feature::Other(n.name.clone()),
        };
        self.warn(
            feature,
            format!(
                "Unsupported element {{{}}}{} omitted or approximated",
                n.ns, n.name
            ),
        );
    }
    pub fn attrs(&mut self, n: &Node, allowed: &[&str], plain: &[&str]) {
        for (ns, key) in n.attrs.keys() {
            let metadata = matches!(
                key.as_str(),
                "rsidR" | "rsidRPr" | "rsidP" | "rsidRDefault" | "rsidSect" | "paraId" | "textId"
            );
            if metadata
                || (matches!(ns.as_str(), W | STRICT_W) && allowed.contains(&key.as_str()))
                || (ns.is_empty() && plain.contains(&key.as_str()))
                || (ns == "http://www.w3.org/XML/1998/namespace" && key == "space")
            {
                continue;
            }
            self.warn(
                Feature::Other("attributes".into()),
                format!("Unsupported attribute {{{ns}}}{key} on {}", n.name),
            );
        }
    }
}
pub(crate) fn number<T: std::str::FromStr>(n: &Node, key: &str) -> Result<Option<T>, DocxError> {
    n.attr(key)
        .map(|v| {
            v.parse()
                .map_err(|_| invalid(format!("invalid {} {key} value {v:?}", n.name)))
        })
        .transpose()
}
pub(crate) fn on(n: &Node) -> Result<bool, DocxError> {
    match n.attr("val").unwrap_or("true") {
        "true" | "1" | "on" => Ok(true),
        "false" | "0" | "off" => Ok(false),
        v => Err(invalid(format!("invalid {} boolean {v:?}", n.name))),
    }
}
pub(crate) fn run_props(
    n: &Node,
    s: &mut TextStyle,
    toggle: bool,
    d: &mut Diagnostics,
) -> Result<(), DocxError> {
    d.attrs(n, &[], &[]);
    for c in &n.children {
        if !(c.ns == W || c.ns == STRICT_W) {
            d.unknown(c);
            continue;
        }
        match c.name.as_str() {
            "b" | "i" => {
                d.attrs(c, &["val"], &[]);
                let v = on(c)?;
                let dest = if c.is("b") {
                    &mut s.bold
                } else {
                    &mut s.italic
                };
                if toggle {
                    if v {
                        *dest = !*dest;
                    }
                } else {
                    *dest = v;
                }
            }
            "u" => {
                d.attrs(c, &["val"], &[]);
                let v = c.attr("val").unwrap_or("single");
                s.underline = v != "none";
                if !matches!(v, "none" | "single") {
                    d.approximate(
                        Feature::Styles,
                        format!("Underline {v} approximated as single"),
                    );
                }
            }
            "rFonts" => {
                d.attrs(c, &["ascii", "hAnsi"], &[]);
                if let Some(font) = c.attr("ascii").or_else(|| c.attr("hAnsi")) {
                    s.font_family = font.into();
                }
                if c.attr("ascii").is_some() != c.attr("hAnsi").is_some() {
                    d.approximate(
                        Feature::Styles,
                        "Only one font slot specified; one font applied to all supported text",
                    );
                }
                if c.attr("hAnsi").is_some_and(|v| v != s.font_family) {
                    d.approximate(
                        Feature::Styles,
                        "Different ASCII and high-ANSI fonts approximated by one font",
                    );
                }
            }
            "sz" => {
                d.attrs(c, &["val"], &[]);
                s.size_half_points =
                    number(c, "val")?.ok_or_else(|| invalid("font size missing val"))?;
            }
            "color" => {
                d.attrs(c, &["val"], &[]);
                let val = c.attr("val").ok_or_else(|| invalid("color missing val"))?;
                if val == "auto" {
                    s.color = Color::BLACK;
                    d.approximate(Feature::Styles, "Automatic color approximated as black");
                } else {
                    if val.len() != 6 || !val.bytes().all(|c| c.is_ascii_hexdigit()) {
                        return Err(invalid("color must be a six-digit RGB value"));
                    }
                    let rgb =
                        u32::from_str_radix(val, 16).map_err(|_| invalid("invalid RGB color"))?;
                    s.color = Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
                }
            }
            "rStyle" => {
                d.attrs(c, &["val"], &[]);
                if c.attr("val").is_none() {
                    return Err(invalid("rStyle missing val"));
                }
            }
            _ => d.unknown(c),
        }
        if !c.children.is_empty() {
            d.warn(
                Feature::Styles,
                format!("Unsupported nested formatting in {}", c.name),
            );
        }
    }
    s.validate()?;
    Ok(())
}
pub(crate) fn para_props(
    n: &Node,
    s: &mut ParagraphStyle,
    d: &mut Diagnostics,
) -> Result<(), DocxError> {
    d.attrs(n, &[], &[]);
    for c in &n.children {
        if !(c.ns == W || c.ns == STRICT_W) {
            d.unknown(c);
            continue;
        }
        match c.name.as_str() {
            "jc" => {
                d.attrs(c, &["val"], &[]);
                s.alignment = match c.attr("val") {
                    Some("left") => Alignment::Left,
                    Some("center") => Alignment::Center,
                    Some("right") => Alignment::Right,
                    Some("both") => Alignment::Justify,
                    Some(v) => {
                        d.approximate(
                            Feature::Styles,
                            format!("Alignment {v} approximated as left"),
                        );
                        Alignment::Left
                    }
                    None => return Err(invalid("alignment missing val")),
                };
            }
            "spacing" => {
                d.attrs(c, &["before", "after", "line", "lineRule"], &[]);
                if let Some(v) = number(c, "before")? {
                    s.space_before_twips = v;
                }
                if let Some(v) = number(c, "after")? {
                    s.space_after_twips = v;
                }
                if c.attr("line").is_some() || c.attr("lineRule").is_some() {
                    let (inherited_line, inherited_rule) = match s.line_spacing {
                        LineSpacing::Multiple(v) => ((u32::from(v) * 240 + 50) / 100, "auto"),
                        LineSpacing::Exact(v) => (v, "exact"),
                        LineSpacing::AtLeast(v) => (v, "atLeast"),
                    };
                    let v = number::<u32>(c, "line")?.unwrap_or(inherited_line);
                    s.line_spacing = match c.attr("lineRule").unwrap_or(inherited_rule) {
                        "auto" => {
                            let percent = (u64::from(v) * 100 + 120) / 240;
                            if u64::from(v) * 100 % 240 != 0 {
                                d.approximate(
                                    Feature::Styles,
                                    "Line spacing rounded to an integer percentage",
                                );
                            }
                            LineSpacing::Multiple(
                                u16::try_from(percent)
                                    .map_err(|_| invalid("line spacing too large"))?,
                            )
                        }
                        "exact" => LineSpacing::Exact(v),
                        "atLeast" => LineSpacing::AtLeast(v),
                        _ => return Err(invalid("invalid lineRule")),
                    };
                }
            }
            "pStyle" | "pageBreakBefore" => {
                d.attrs(c, &["val"], &[]);
                if c.is("pStyle") && c.attr("val").is_none() {
                    return Err(invalid("pStyle missing val"));
                }
            }
            "rPr" => {} // Paragraph-mark properties handled separately.
            "sectPr" => d.warn(
                Feature::Sections,
                "Per-paragraph section settings omitted; final document layout used",
            ),
            _ => d.unknown(c),
        }
    }
    s.validate()?;
    Ok(())
}
#[derive(Default)]
pub(crate) struct Styles {
    definitions: BTreeMap<String, Node>,
    pub default_p: Option<String>,
    pub default_r: Option<String>,
    pub run: TextStyle,
    pub paragraph: ParagraphStyle,
}
impl Styles {
    pub fn parse(root: Node, d: &mut Diagnostics) -> Result<Self, DocxError> {
        if !root.is("styles") {
            return Err(invalid("styles part has wrong root"));
        }
        d.attrs(&root, &[], &[]);
        let mut styles = Self::default();
        for n in root.children {
            if n.is("docDefaults") {
                for c in &n.children {
                    if c.is("rPrDefault") {
                        if let Some(r) = c.child("rPr") {
                            run_props(r, &mut styles.run, false, d)?;
                        }
                    } else if c.is("pPrDefault") {
                        if let Some(p) = c.child("pPr") {
                            para_props(p, &mut styles.paragraph, d)?;
                        }
                    } else {
                        d.unknown(c);
                    }
                }
            } else if n.is("style") {
                d.attrs(&n, &["styleId", "type", "default", "customStyle"], &[]);
                let id = n
                    .attr("styleId")
                    .ok_or_else(|| invalid("style missing styleId"))?
                    .to_owned();
                if !matches!(n.attr("type"), Some("paragraph" | "character")) {
                    d.warn(Feature::Styles, format!("Unsupported style type for {id}"));
                }
                if n.attr("default").is_some() && on_default(&n)? {
                    match n.attr("type") {
                        Some("paragraph") => styles.default_p = Some(id.clone()),
                        Some("character") => styles.default_r = Some(id.clone()),
                        _ => {}
                    }
                }
                for c in &n.children {
                    if ![
                        "basedOn",
                        "pPr",
                        "rPr",
                        "name",
                        "next",
                        "link",
                        "qFormat",
                        "uiPriority",
                        "semiHidden",
                        "unhideWhenUsed",
                        "aliases",
                        "locked",
                        "autoRedefine",
                        "personal",
                        "personalCompose",
                        "personalReply",
                        "rsid",
                    ]
                    .iter()
                    .any(|s| c.is(s))
                    {
                        d.unknown(c);
                    }
                }
                if let Some(base) = n.child("basedOn")
                    && base.attr("val").is_none()
                {
                    return Err(invalid("basedOn missing val"));
                }
                if styles.definitions.len() >= 4096 {
                    return Err(DocxError::ResourceLimit(
                        "styles exceeds 4,096 definitions".into(),
                    ));
                }
                if styles.definitions.insert(id, n).is_some() {
                    return Err(invalid("duplicate styleId"));
                }
            } else if n.is("latentStyles") { /* UI metadata, not rendering */
            } else {
                d.unknown(&n);
            }
        }
        // Diagnose resolved properties, rather than evaluating partial styles in
        // isolation (an omitted lineRule may inherit exact/atLeast).
        for id in styles.definitions.keys() {
            let mut run = styles.run.clone();
            let mut paragraph = styles.paragraph.clone();
            styles.apply(id, &mut run, Some(&mut paragraph), d)?;
        }
        Ok(styles)
    }
    fn chain(&self, id: &str) -> Result<Vec<&Node>, DocxError> {
        let mut chain = Vec::new();
        let mut seen = Vec::new();
        let mut current = Some(id);
        while let Some(id) = current {
            if seen.contains(&id) {
                return Err(invalid(format!("cyclic style inheritance at {id}")));
            }
            if seen.len() >= 64 {
                return Err(DocxError::ResourceLimit(
                    "style inheritance exceeds 64".into(),
                ));
            }
            seen.push(id);
            let Some(n) = self.definitions.get(id) else {
                break;
            };
            chain.push(n);
            current = n.child("basedOn").and_then(|b| b.attr("val"));
        }
        chain.reverse();
        Ok(chain)
    }
    pub fn page_break_before(&self, id: &str) -> Result<Option<bool>, DocxError> {
        let mut result = None;
        for n in self.chain(id)? {
            if let Some(p) = n.child("pPr").and_then(|p| p.child("pageBreakBefore")) {
                result = Some(on(p)?);
            }
        }
        Ok(result)
    }
    pub fn apply(
        &self,
        id: &str,
        run: &mut TextStyle,
        mut paragraph: Option<&mut ParagraphStyle>,
        d: &mut Diagnostics,
    ) -> Result<(), DocxError> {
        let chain = self.chain(id)?;
        if chain.is_empty() {
            d.missing_style(format!("Missing referenced style {id}; defaults used"));
        }
        for n in chain {
            if let Some(base) = n.child("basedOn").and_then(|b| b.attr("val"))
                && !self.definitions.contains_key(base)
            {
                d.missing_style(format!("Missing base style {base}; defaults used"));
            }
            if let Some(p) = n.child("pPr")
                && let Some(style) = paragraph.as_deref_mut()
            {
                para_props(p, style, d)?;
            }
            if let Some(r) = n.child("rPr") {
                run_props(r, run, true, d)?;
            }
        }
        Ok(())
    }
}
fn on_default(n: &Node) -> Result<bool, DocxError> {
    match n.attr("default").unwrap_or("false") {
        "1" | "true" | "on" => Ok(true),
        "0" | "false" | "off" => Ok(false),
        _ => Err(invalid("invalid default style boolean")),
    }
}
