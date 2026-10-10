//! Provider-independent MCP over stdio, with an authenticated loopback bridge
//! for the visible editor. Tool execution always stays on the editor's thread.
use crate::{FolioApp, editing};
use document_core::*;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

const MAX_MESSAGE: u64 = 4 * 1024 * 1024;
const VERSIONS: &[&str] = &[
    "2026-07-28",
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
];
const INSTRUCTIONS: &str = "Use folio_get_document before editing. Positions are zero-based block indices and UTF-8 byte offsets at grapheme boundaries, not character indices. AI edits share the normal undo history. Save explicitly before ending a background session. Never discard unsaved work or overwrite a file without the user's instruction.";

fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}

pub fn tools() -> Vec<Value> {
    let position = object(
        json!({"block":{"type":"integer","minimum":0},"offset":{"type":"integer","minimum":0}}),
        &["block", "offset"],
    );
    let selection = object(
        json!({"anchor":position,"focus":position}),
        &["anchor", "focus"],
    );
    let boolean = json!({"type":"boolean"});
    let string = json!({"type":"string"});
    let rgb = object(
        json!({"red":{"type":"integer","minimum":0,"maximum":255},"green":{"type":"integer","minimum":0,"maximum":255},"blue":{"type":"integer","minimum":0,"maximum":255}}),
        &["red", "green", "blue"],
    );
    let line_spacing = json!({"oneOf":[
        object(json!({"kind":{"type":"string","const":"multiple"},"value":{"type":"integer","minimum":1,"maximum":65535}}), &["kind","value"]),
        object(json!({"kind":{"type":"string","enum":["exact","at_least"]},"value":{"type":"integer","minimum":1,"maximum":4294967295_u64}}), &["kind","value"])
    ]});
    let distance = json!({"type":"integer","minimum":0,"maximum":4294967295_u64});
    let dimension = json!({"type":"integer","minimum":1,"maximum":4294967295_u64});
    let layout = object(
        json!({"size":object(json!({"width_twips":dimension,"height_twips":dimension}), &["width_twips","height_twips"]),"orientation":{"type":"string","enum":["portrait","landscape"]},"margins":object(json!({"top":distance,"right":distance,"bottom":distance,"left":distance}), &["top","right","bottom","left"])}),
        &["size", "orientation", "margins"],
    );
    let match_case = json!({"type":"boolean","default":true});
    let whole_words = json!({"type":"boolean","default":false});
    let definitions = vec![
        (
            "folio_convert_case",
            "Convert a nonempty selection using locale-independent Unicode upper/lower/title/sentence casing, preserving runs and paragraph structure. Title uses whitespace-delimited tokens; sentence starts follow punctuation and whitespace.",
            object(
                json!({"selection":selection,"case":{"type":"string","enum":["upper","lower","title","sentence"]}}),
                &["selection", "case"],
            ),
            false,
        ),
        (
            "folio_set_page_layout",
            "Set complete page layout. Nominal dimensions and margins are twips (1440 per inch); landscape swaps effective dimensions. Margins must leave positive content area.",
            object(json!({"layout":layout}), &["layout"]),
            false,
        ),
        (
            "folio_list_templates",
            "List stable document template IDs, names and descriptions.",
            object(json!({}), &[]),
            true,
        ),
        (
            "folio_duplicate_document",
            "Save a distinct absolute DOCX copy without changing the active document, history or path. Existing destinations require overwrite=true; source aliases remain protected.",
            object(
                json!({"path":string,"overwrite":{"type":"boolean","default":false}}),
                &["path"],
            ),
            false,
        ),
        (
            "folio_export_text",
            "Export the document or optional selection to a distinct absolute UTF-8 .txt path without changing editor state. Paragraphs use newlines; page breaks use form feeds. Existing destinations require overwrite=true.",
            object(
                json!({"path":string,"selection":selection,"overwrite":{"type":"boolean","default":false}}),
                &["path"],
            ),
            false,
        ),
        (
            "folio_get_document",
            "Read indexed blocks with complete run/default and paragraph styles, full page_layout, read_only, statistics, file path and dirty state.",
            object(json!({}), &[]),
            true,
        ),
        (
            "folio_get_selection",
            "Read the current selection and its text.",
            object(json!({}), &[]),
            true,
        ),
        (
            "folio_select",
            "Set an explicit Unicode-safe selection.",
            object(json!({"selection":selection}), &["selection"]),
            false,
        ),
        (
            "folio_replace_text",
            "Insert, replace or delete text at an explicit selection. Newlines create paragraphs. Optional expected_text prevents editing a changed range.",
            object(
                json!({"selection":selection,"text":string,"expected_text":string}),
                &["selection", "text"],
            ),
            false,
        ),
        (
            "folio_format_text",
            "Patch an explicit nonempty selection. Font sizes are half-points (24 = 12pt); RGB channels are 0..255. Omitted highlight preserves it; highlight:null clears it. Vertical alignment is baseline/superscript/subscript.",
            object(
                json!({"selection":selection,"bold":boolean,"italic":boolean,"underline":boolean,"font_family":string,"size_half_points":{"type":"integer","minimum":1,"maximum":65535},"strikethrough":boolean,"vertical_align":{"type":"string","enum":["baseline","superscript","subscript"]},"color":rgb,"highlight":{"anyOf":[rgb,{"type":"null"}]}}),
                &["selection"],
            ),
            false,
        ),
        (
            "folio_format_paragraph",
            "Patch paragraph alignment and spacing. Before/after use nonnegative twips; line spacing uses positive percentages for multiple, positive twips for exact/at_least.",
            object(
                json!({"selection":selection,"alignment":{"type":"string","enum":["left","center","right","justify"]},"space_before_twips":distance,"space_after_twips":distance,"line_spacing":line_spacing}),
                &["selection"],
            ),
            false,
        ),
        (
            "folio_find",
            "Find literal paragraph-local text at grapheme boundaries. match_case defaults true; whole_words defaults false and uses Unicode word boundaries.",
            object(
                json!({"text":string,"match_case":match_case,"whole_words":whole_words}),
                &["text"],
            ),
            true,
        ),
        (
            "folio_replace_all",
            "Replace paragraph-local literal matches as one undoable edit. match_case defaults true; whole_words defaults false. Replacement newlines create paragraphs.",
            object(
                json!({"text":string,"replacement":string,"match_case":match_case,"whole_words":whole_words}),
                &["text", "replacement"],
            ),
            false,
        ),
        (
            "folio_insert_page_break",
            "Replace an explicit selection with a page break.",
            object(json!({"selection":selection}), &["selection"]),
            false,
        ),
        (
            "folio_undo",
            "Undo the latest edit (including edits made in the UI).",
            object(json!({}), &[]),
            false,
        ),
        (
            "folio_redo",
            "Redo the latest undone edit.",
            object(json!({}), &[]),
            false,
        ),
        (
            "folio_new_document",
            "Start a blank document or named template. Nonblank templates start unsaved. Refuses dirty content unless discard_unsaved=true; successful switch leaves read-only mode.",
            object(
                json!({"discard_unsaved":boolean,"template":{"type":"string","enum":["blank","letter","meeting_notes","project_brief"]}}),
                &[],
            ),
            false,
        ),
        (
            "folio_open_document",
            "Open an absolute DOCX path, returning import warnings. Refuses unsaved changes unless explicitly allowed.",
            object(json!({"path":string,"discard_unsaved":boolean}), &["path"]),
            false,
        ),
        (
            "folio_save_document",
            "Atomically save to an absolute DOCX path. Existing files require overwrite=true. Warned import sources remain protected; read-only mode requires a distinct destination.",
            object(json!({"path":string,"overwrite":boolean}), &["path"]),
            false,
        ),
    ];
    definitions.into_iter().map(|(name, description, input_schema, readonly)| json!({
        "name":name,"description":description,"inputSchema":input_schema,
        "annotations":{"readOnlyHint":readonly,"destructiveHint":!readonly,"openWorldHint":false}
    })).collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    block: usize,
    offset: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Range {
    anchor: Point,
    focus: Point,
}
impl Range {
    fn selection(self) -> Selection {
        Selection::new(
            Position::new(self.anchor.block, self.anchor.offset),
            Position::new(self.focus.block, self.focus.offset),
        )
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Select {
    selection: Range,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replace {
    selection: Range,
    text: String,
    #[serde(default, deserialize_with = "present_value")]
    expected_text: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Format {
    selection: Range,
    #[serde(default, deserialize_with = "present_value")]
    bold: Option<bool>,
    #[serde(default, deserialize_with = "present_value")]
    italic: Option<bool>,
    #[serde(default, deserialize_with = "present_value")]
    underline: Option<bool>,
    #[serde(default, deserialize_with = "present_value")]
    font_family: Option<String>,
    #[serde(default, deserialize_with = "present_value")]
    size_half_points: Option<u16>,
    #[serde(default, deserialize_with = "present_value")]
    strikethrough: Option<bool>,
    #[serde(default, deserialize_with = "present_value")]
    vertical_align: Option<VerticalAlign>,
    #[serde(default, deserialize_with = "present_value")]
    color: Option<RgbInput>,
    #[serde(default, deserialize_with = "present_nullable_rgb")]
    highlight: Option<Option<RgbInput>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParagraphFormat {
    selection: Range,
    #[serde(default, deserialize_with = "present_value")]
    alignment: Option<String>,
    #[serde(default, deserialize_with = "present_value")]
    space_before_twips: Option<u32>,
    #[serde(default, deserialize_with = "present_value")]
    space_after_twips: Option<u32>,
    #[serde(default, deserialize_with = "present_value")]
    line_spacing: Option<LineSpacingInput>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    text: String,
    #[serde(default = "default_true")]
    match_case: bool,
    #[serde(default)]
    whole_words: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplaceAll {
    text: String,
    replacement: String,
    #[serde(default = "default_true")]
    match_case: bool,
    #[serde(default)]
    whole_words: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct New {
    #[serde(default, deserialize_with = "present_value")]
    template: Option<String>,
    #[serde(default)]
    discard_unsaved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Open {
    path: PathBuf,
    #[serde(default)]
    discard_unsaved: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Save {
    path: PathBuf,
    #[serde(default)]
    overwrite: bool,
}

// Optional fields reject explicit null unless the public schema permits it.
fn present_value<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
fn default_true() -> bool {
    true
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RgbInput {
    red: u8,
    green: u8,
    blue: u8,
}
impl From<RgbInput> for Color {
    fn from(v: RgbInput) -> Self {
        Self::rgb(v.red, v.green, v.blue)
    }
}
fn present_nullable_rgb<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<RgbInput>>, D::Error> {
    Option::<RgbInput>::deserialize(d).map(Some)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LineSpacingInput {
    kind: String,
    value: u32,
}
impl LineSpacingInput {
    fn spacing(self) -> Result<LineSpacing, String> {
        if self.value == 0 {
            return Err("Line spacing must be positive".into());
        }
        match self.kind.as_str() {
            "multiple" => u16::try_from(self.value)
                .map(LineSpacing::Multiple)
                .map_err(|_| "Multiple spacing must be 1..65535 percent".into()),
            "exact" => Ok(LineSpacing::Exact(self.value)),
            "at_least" => Ok(LineSpacing::AtLeast(self.value)),
            _ => Err("Unknown line spacing kind".into()),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseInput {
    selection: Range,
    case: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SizeInput {
    width_twips: u32,
    height_twips: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MarginsInput {
    top: u32,
    right: u32,
    bottom: u32,
    left: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayoutInput {
    size: SizeInput,
    orientation: String,
    margins: MarginsInput,
}
impl LayoutInput {
    fn layout(self) -> Result<PageLayout, String> {
        Ok(PageLayout {
            size: PageSize {
                width_twips: self.size.width_twips,
                height_twips: self.size.height_twips,
            },
            orientation: match self.orientation.as_str() {
                "portrait" => Orientation::Portrait,
                "landscape" => Orientation::Landscape,
                _ => return Err("Unknown orientation".into()),
            },
            margins: Margins {
                top: self.margins.top,
                right: self.margins.right,
                bottom: self.margins.bottom,
                left: self.margins.left,
            },
        })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageInput {
    layout: LayoutInput,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportInput {
    path: PathBuf,
    #[serde(default, deserialize_with = "present_value")]
    selection: Option<Range>,
    #[serde(default)]
    overwrite: bool,
}
fn paragraph_style(s: &ParagraphStyle) -> Value {
    let (kind, value) = match s.line_spacing {
        LineSpacing::Multiple(v) => ("multiple", u32::from(v)),
        LineSpacing::Exact(v) => ("exact", v),
        LineSpacing::AtLeast(v) => ("at_least", v),
    };
    json!({"alignment":format!("{:?}",s.alignment).to_lowercase(),"space_before_twips":s.space_before_twips,"space_after_twips":s.space_after_twips,"line_spacing":{"kind":kind,"value":value}})
}
fn page_layout(p: &PageLayout) -> Value {
    json!({"size":p.size,"orientation":format!("{:?}",p.orientation).to_lowercase(),"margins":p.margins})
}
fn args<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid arguments: {e}"))
}
fn range(selection: Selection) -> Value {
    json!({"anchor":{"block":selection.anchor.block,"offset":selection.anchor.offset},"focus":{"block":selection.focus.block,"offset":selection.focus.offset}})
}
fn style(s: &TextStyle) -> Value {
    json!({"bold":s.bold,"italic":s.italic,"underline":s.underline,"strikethrough":s.strikethrough,"vertical_align":s.vertical_align,"highlight":s.highlight,"font_family":s.font_family,"size_half_points":s.size_half_points,"color":{"red":s.color.red,"green":s.color.green,"blue":s.color.blue}})
}
fn document(app: &FolioApp) -> Value {
    let blocks: Vec<_> = app.editor.document().blocks.iter().enumerate().map(|(index, b)| match b {
        Block::PageBreak => json!({"index":index,"type":"page_break"}),
        Block::Paragraph(p) => json!({"index":index,"type":"paragraph","text":p.text(),"byte_length":p.len_bytes(),"alignment":format!("{:?}",p.style.alignment).to_lowercase(),"style":paragraph_style(&p.style),"default_style":style(&p.default_style),"runs":p.runs.iter().map(|r|json!({"text":r.text,"style":style(&r.style)})).collect::<Vec<_>>()})
    }).collect();
    let stats = editing::document_statistics(app.editor.document());
    json!({"blocks":blocks,"page_layout":page_layout(&app.editor.document().page_layout),"read_only":app.read_only,"selection":range(app.editor.selection()),"path":app.path.as_ref().map(|p|p.to_string_lossy()),"dirty":app.editor.is_dirty(),"can_undo":app.editor.can_undo(),"can_redo":app.editor.can_redo(),"statistics":{"words":stats.words,"characters":stats.characters},"warnings":app.warnings.iter().map(|w|&w.message).collect::<Vec<_>>()})
}
fn check_destination_path(path: &std::path::Path, extension: &str) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
    {
        return Err(format!("Use a nonempty absolute .{extension} path"));
    }
    Ok(())
}
fn check_unsaved(app: &FolioApp, discard: bool) -> Result<(), String> {
    if app.editor.is_dirty() && !discard {
        return Err("Unsaved changes: save first, or explicitly set discard_unsaved=true".into());
    }
    Ok(())
}

pub fn call(app: &mut FolioApp, name: &str, arguments: Value) -> Result<Value, String> {
    if app.pending_recovery.is_some()
        || app.pending.is_some()
        || app.overwrite.is_some()
        || app.error.is_some()
    {
        return Err("Resolve the open Folio dialog before using MCP tools".into());
    }
    let reads = matches!(
        name,
        "folio_get_document" | "folio_get_selection" | "folio_find" | "folio_list_templates"
    );
    if !reads && (app.workbench_blocks_editing() || app.template_gallery) {
        return Err("Resolve the open Folio workbench dialog before using this MCP tool".into());
    }
    let command = match name {
        "folio_list_templates" => {
            let _: Empty = args(arguments)?;
            return Ok(
                json!({"templates":crate::templates::CATALOG.iter().map(|&(_,id,name,description)|json!({"id":id,"name":name,"description":description})).collect::<Vec<_>>()}),
            );
        }
        "folio_convert_case" => {
            let a: CaseInput = args(arguments)?;
            let case = match a.case.as_str() {
                "upper" => TextCase::Upper,
                "lower" => TextCase::Lower,
                "title" => TextCase::Title,
                "sentence" => TextCase::Sentence,
                _ => return Err("Unknown case".into()),
            };
            let selection = a.selection.selection();
            if selection.is_collapsed() {
                return Err("Select text to convert case".into());
            }
            Command::ConvertCase { selection, case }
        }
        "folio_set_page_layout" => {
            let a: PageInput = args(arguments)?;
            Command::SetPageLayout {
                layout: a.layout.layout()?,
            }
        }
        "folio_duplicate_document" => {
            let a: Save = args(arguments)?;
            check_destination_path(&a.path, "docx")?;
            app.duplicate_to(a.path.clone(), a.overwrite)?;
            return Ok(json!({"duplicated":true,"path":a.path}));
        }
        "folio_export_text" => {
            let a: ExportInput = args(arguments)?;
            check_destination_path(&a.path, "txt")?;
            app.export_text_to(
                a.path.clone(),
                a.selection.map(Range::selection),
                a.overwrite,
            )?;
            return Ok(json!({"exported":true,"path":a.path}));
        }
        "folio_get_document" => {
            let _: Empty = args(arguments)?;
            return Ok(document(app));
        }
        "folio_get_selection" => {
            let _: Empty = args(arguments)?;
            return Ok(
                json!({"selection":range(app.editor.selection()),"text":editing::selected_text(&app.editor)}),
            );
        }
        "folio_find" => {
            let a: Search = args(arguments)?;
            return app
                .editor
                .document()
                .find_with_options(
                    &a.text,
                    SearchOptions {
                        match_case: a.match_case,
                        whole_words: a.whole_words,
                    },
                )
                .map(
                    |matches| json!({"matches":matches.into_iter().map(range).collect::<Vec<_>>()}),
                )
                .map_err(|e| e.to_string());
        }
        "folio_select" => {
            let a: Select = args(arguments)?;
            app.editor
                .set_selection(a.selection.selection())
                .map_err(|e| e.to_string())?;
            reset_edit_state(app);
            return Ok(json!({"selection":range(app.editor.selection())}));
        }
        "folio_replace_text" => {
            let a: Replace = args(arguments)?;
            let selection = a.selection.selection();
            if let Some(expected) = a.expected_text {
                let mut reader = app.editor.clone();
                reader.set_selection(selection).map_err(|e| e.to_string())?;
                if editing::selected_text(&reader) != expected {
                    return Err("The range changed: read the document again before editing".into());
                }
            }
            Command::ReplaceText {
                selection,
                text: a.text,
                style: None,
            }
        }
        "folio_format_text" => {
            let a: Format = args(arguments)?;
            let selection = a.selection.selection();
            if selection.is_collapsed() {
                return Err("Select text to format".into());
            }
            Command::FormatRuns {
                selection,
                patch: StylePatch {
                    bold: a.bold,
                    italic: a.italic,
                    underline: a.underline,
                    font_family: a.font_family,
                    size_half_points: a.size_half_points,
                    strikethrough: a.strikethrough,
                    vertical_align: a.vertical_align,
                    color: a.color.map(Color::from),
                    highlight: a.highlight.map(|v| v.map(Color::from)),
                },
            }
        }
        "folio_format_paragraph" => {
            let a: ParagraphFormat = args(arguments)?;
            let alignment = a
                .alignment
                .map(|v| match v.as_str() {
                    "left" => Ok(Alignment::Left),
                    "center" => Ok(Alignment::Center),
                    "right" => Ok(Alignment::Right),
                    "justify" => Ok(Alignment::Justify),
                    _ => Err("Unknown alignment".to_string()),
                })
                .transpose()?;
            Command::FormatParagraphs {
                selection: a.selection.selection(),
                patch: ParagraphPatch {
                    alignment,
                    space_before_twips: a.space_before_twips,
                    space_after_twips: a.space_after_twips,
                    line_spacing: a.line_spacing.map(LineSpacingInput::spacing).transpose()?,
                },
            }
        }
        "folio_replace_all" => {
            let a: ReplaceAll = args(arguments)?;
            Command::ReplaceAllWithOptions {
                needle: a.text,
                replacement: a.replacement,
                options: SearchOptions {
                    match_case: a.match_case,
                    whole_words: a.whole_words,
                },
            }
        }
        "folio_insert_page_break" => {
            let a: Select = args(arguments)?;
            Command::ReplaceWithPageBreak {
                selection: a.selection.selection(),
            }
        }
        "folio_undo" => {
            let _: Empty = args(arguments)?;
            Command::Undo
        }
        "folio_redo" => {
            let _: Empty = args(arguments)?;
            Command::Redo
        }
        "folio_new_document" => {
            let a: New = args(arguments)?;
            check_unsaved(app, a.discard_unsaved)?;
            let id = match a.template {
                None => crate::templates::TemplateId::Blank,
                Some(id) => crate::templates::CATALOG
                    .iter()
                    .find(|entry| entry.1 == id)
                    .map(|entry| entry.0)
                    .ok_or("Unknown template")?,
            };
            app.template_document(id)?;
            reset_edit_state(app);
            return Ok(document(app));
        }
        "folio_open_document" => {
            let a: Open = args(arguments)?;
            check_destination_path(&a.path, "docx")?;
            check_unsaved(app, a.discard_unsaved)?;
            app.open_document(a.path)?;
            reset_edit_state(app);
            return Ok(document(app));
        }
        "folio_save_document" => {
            let a: Save = args(arguments)?;
            check_destination_path(&a.path, "docx")?;
            if a.path.exists() && !a.overwrite {
                return Err(
                    "Destination exists: explicitly set overwrite=true to replace it".into(),
                );
            }
            app.save_document(a.path)?;
            app.notice = "Saved through MCP".into();
            return Ok(json!({"saved":true,"path":app.path.as_ref().map(|p|p.to_string_lossy())}));
        }
        _ => return Err(format!("Unknown tool: {name}")),
    };
    if app.is_mutation_blocked() {
        return Err("Document mutation is blocked by read-only mode or an open dialog".into());
    }
    let outcome = app.editor.execute(command).map_err(|e| e.to_string())?;
    reset_edit_state(app);
    Ok(
        json!({"changed":outcome.changed,"replacements":outcome.replacements,"selection":range(outcome.selection),"dirty":app.editor.is_dirty()}),
    )
}
fn reset_edit_state(app: &mut FolioApp) {
    app.typing = None;
    app.composition = None;
    app.ime_enabled = false;
    app.visual_line = None;
    app.preferred_x = None;
    app.reveal = true;
    app.notice = "Updated through MCP".into();
}

#[derive(Default)]
struct Protocol {
    initialized: bool,
    ready: bool,
}
fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
impl Protocol {
    fn handle(
        &mut self,
        message: Value,
        call: &mut impl FnMut(&str, Value) -> Result<Value, String>,
    ) -> Option<Value> {
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        if message["jsonrpc"] != "2.0"
            || !message["method"].is_string()
            || (!id.is_null() && !id.is_string() && !id.is_number())
        {
            return Some(error(id, -32600, "Invalid JSON-RPC request"));
        }
        let method = message["method"].as_str().unwrap();
        if message.get("id").is_none() {
            if method == "notifications/initialized" && self.initialized {
                self.ready = true;
            }
            return None;
        }
        if id.is_null() {
            return Some(error(id, -32600, "Request ID must be a string or number"));
        }
        let params = message.get("params").cloned().unwrap_or(json!({}));
        if !params.is_object() {
            return Some(error(id, -32602, "params must be an object"));
        }
        let meta_version = params["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str();
        let modern = meta_version == Some(VERSIONS[0]);
        if let Some(version) = meta_version
            && !VERSIONS.contains(&version)
        {
            let mut response = error(id, -32022, "Unsupported protocol version");
            response["error"]["data"] = json!({"supported":VERSIONS,"requested":version});
            return Some(response);
        }
        if modern && !params["_meta"]["io.modelcontextprotocol/clientCapabilities"].is_object() {
            return Some(error(
                id,
                -32602,
                "Modern requests require clientCapabilities metadata",
            ));
        }
        let result = match method {
            "initialize" => {
                if self.initialized {
                    return Some(error(id, -32600, "Already initialized"));
                }
                let Some(requested) = params["protocolVersion"].as_str() else {
                    return Some(error(id, -32602, "Missing protocolVersion"));
                };
                if !params["capabilities"].is_object()
                    || !params["clientInfo"]["name"].is_string()
                    || !params["clientInfo"]["version"].is_string()
                {
                    return Some(error(id, -32602, "Missing capabilities or clientInfo"));
                }
                let version = if VERSIONS[1..].contains(&requested) {
                    requested
                } else {
                    VERSIONS[1]
                };
                self.initialized = true;
                json!({"protocolVersion":version,"capabilities":{"tools":{}},"serverInfo":{"name":"folio","version":env!("CARGO_PKG_VERSION")},"instructions":INSTRUCTIONS})
            }
            "server/discover" if modern => {
                json!({"supportedVersions":VERSIONS,"capabilities":{"tools":{}},"instructions":INSTRUCTIONS})
            }
            "ping" => json!({}),
            _ if !self.ready && !modern => {
                return Some(error(id, -32000, "Initialize the MCP session first"));
            }
            "tools/list" => {
                if params.get("cursor").is_some() {
                    return Some(error(id, -32602, "No pagination cursor is supported"));
                }
                json!({"tools":tools()})
            }
            "tools/call" => {
                let Some(name) = params["name"].as_str() else {
                    return Some(error(id, -32602, "Missing tool name"));
                };
                if !tools().iter().any(|t| t["name"] == name) {
                    return Some(error(id, -32602, "Unknown tool"));
                }
                let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
                match call(name, arguments) {
                    Ok(data) => {
                        json!({"content":[{"type":"text","text":data.to_string()}],"structuredContent":data,"isError":false})
                    }
                    Err(message) => {
                        json!({"content":[{"type":"text","text":message}],"isError":true})
                    }
                }
            }
            _ => return Some(error(id, -32601, "Method not found")),
        };
        let mut result = result;
        if modern {
            result["resultType"] = json!("complete");
            result["_meta"] = json!({"io.modelcontextprotocol/serverInfo":{"name":"folio","version":env!("CARGO_PKG_VERSION")}});
        }
        Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
    }
}

fn read_message(input: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    let n = input.take(MAX_MESSAGE + 1).read_until(b'\n', &mut line)?;
    if n == 0 {
        return Ok(None);
    }
    if n as u64 > MAX_MESSAGE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MCP message exceeds 4 MiB",
        ));
    }
    Ok(Some(line))
}
fn write_message(output: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    output.write_all(b"\n")?;
    output.flush()
}
fn serve(
    input: &mut impl BufRead,
    output: &mut impl Write,
    mut call: impl FnMut(&str, Value) -> Result<Value, String>,
) -> io::Result<()> {
    let mut protocol = Protocol::default();
    while let Some(line) = read_message(input)? {
        let response = match serde_json::from_slice(&line) {
            Ok(message) => protocol.handle(message, &mut call),
            Err(_) => Some(error(Value::Null, -32700, "Invalid JSON")),
        };
        if let Some(response) = response {
            write_message(output, &response)?;
        }
    }
    Ok(())
}
pub fn background() -> io::Result<()> {
    let mut app = FolioApp::default();
    serve(
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
        |name, args| call(&mut app, name, args),
    )
}

pub struct Request {
    pub name: String,
    pub args: Value,
    pub reply: mpsc::Sender<Result<Value, String>>,
}
pub struct Bridge {
    pub receiver: mpsc::Receiver<Request>,
    pub address: SocketAddr,
    token: String,
    stopped: Arc<AtomicBool>,
    sockets: Arc<Mutex<Vec<TcpStream>>>,
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Ok(sockets) = self.sockets.lock() {
            for socket in sockets.iter() {
                let _ = socket.shutdown(Shutdown::Both);
            }
        }
    }
}
impl Bridge {
    pub fn start(ctx: egui::Context) -> io::Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let token = rand::random::<[u8; 32]>()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let (sender, receiver) = mpsc::sync_channel::<Request>(8);
        let stopped = Arc::new(AtomicBool::new(false));
        let sockets = Arc::new(Mutex::new(Vec::<TcpStream>::new()));
        let stop = stopped.clone();
        let connections = sockets.clone();
        let secret = token.clone();
        thread::spawn(move || {
            while !stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((socket, _)) => {
                        let mut active = connections.lock().unwrap();
                        active.retain(|s| s.peer_addr().is_ok());
                        if active.len() >= 8 {
                            let _ = socket.shutdown(Shutdown::Both);
                            continue;
                        }
                        let Ok(copy) = socket.try_clone() else {
                            continue;
                        };
                        active.push(copy);
                        drop(active);
                        let sender = sender.clone();
                        let secret = secret.clone();
                        let ctx = ctx.clone();
                        let stop = stop.clone();
                        let connections = connections.clone();
                        thread::spawn(move || {
                            let peer = socket.peer_addr().ok();
                            if let Err(error) =
                                live_connection(socket, &secret, &sender, &ctx, &stop)
                            {
                                eprintln!("Folio live connection: {error}");
                            }
                            connections
                                .lock()
                                .unwrap()
                                .retain(|s| s.peer_addr().ok() != peer);
                        });
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            receiver,
            address,
            token,
            stopped,
            sockets,
        })
    }
    pub fn configuration(&self) -> Value {
        json!({"mcpServers":{"folio-live":{"command":std::env::current_exe().unwrap_or_else(|_|PathBuf::from("folio-desktop")).to_string_lossy(),"args":["--mcp-connect",self.address.to_string()],"env":{"FOLIO_MCP_TOKEN":self.token}}}})
    }
}
fn live_connection(
    mut socket: TcpStream,
    token: &str,
    sender: &mpsc::SyncSender<Request>,
    ctx: &egui::Context,
    stopped: &AtomicBool,
) -> io::Result<()> {
    // On macOS accepted sockets inherit the listener's nonblocking mode.
    // Workers use blocking framed reads; only the accept loop is nonblocking.
    socket.set_nonblocking(false)?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut input = BufReader::new(socket.try_clone()?);
    let line = read_message(&mut input)?.ok_or(io::ErrorKind::UnexpectedEof)?;
    let auth: Value = serde_json::from_slice(&line)?;
    if auth["token"] != token || stopped.load(Ordering::Acquire) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "AI connection is disabled or token is invalid",
        ));
    }
    write_message(&mut socket, &json!({"authenticated":true}))?;
    socket.set_read_timeout(None)?;
    serve(&mut input, &mut socket, |name, args| {
        if stopped.load(Ordering::Acquire) {
            return Err("AI connection disabled".into());
        }
        let (reply, answer) = mpsc::channel();
        sender
            .try_send(Request {
                name: name.into(),
                args,
                reply,
            })
            .map_err(|_| "Folio is busy or disconnected".to_string())?;
        ctx.request_repaint();
        answer
            .recv()
            .map_err(|_| "Folio disconnected".to_string())?
    })
}
pub fn connect(address: &str, token: &str) -> io::Result<()> {
    let address: SocketAddr = address.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid live connection address",
        )
    })?;
    if !address.ip().is_loopback() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Only loopback connections are supported",
        ));
    }
    let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    write_message(&mut socket, &json!({"token":token}))?;
    let mut input = BufReader::new(socket.try_clone()?);
    let auth = read_message(&mut input)?.ok_or(io::ErrorKind::UnexpectedEof)?;
    if serde_json::from_slice::<Value>(&auth)?["authenticated"] != true {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    socket.set_read_timeout(None)?;
    let outbound = socket.try_clone()?;
    thread::spawn(move || {
        let _ = io::copy(&mut io::stdin().lock(), &mut &outbound);
        let _ = outbound.shutdown(Shutdown::Write);
    });
    io::copy(&mut input, &mut io::stdout().lock())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn selection(a: usize, b: usize) -> Value {
        json!({"anchor":{"block":0,"offset":a},"focus":{"block":0,"offset":b}})
    }
    #[test]
    fn parity_schema_styles_options_case_and_layout() {
        assert_eq!(tools().len(), 19);
        let mut app = FolioApp::default();
        call(
            &mut app,
            "folio_replace_text",
            json!({"selection":selection(0,0),"text":"Straße CAT cat cats"}),
        )
        .unwrap();
        assert_eq!(
            call(&mut app, "folio_find", json!({"text":"cat"})).unwrap()["matches"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            call(
                &mut app,
                "folio_find",
                json!({"text":"cat","match_case":false,"whole_words":true})
            )
            .unwrap()["matches"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        call(&mut app,"folio_format_text",json!({"selection":selection(0,7),"color":{"red":1,"green":2,"blue":3},"highlight":{"red":255,"green":255,"blue":0},"strikethrough":true,"vertical_align":"superscript"})).unwrap();
        let applied = &app.editor.document().paragraph(0).unwrap().runs[0].style;
        assert_eq!(applied.color, Color::rgb(1, 2, 3));
        assert!(applied.strikethrough);
        assert_eq!(applied.vertical_align, VerticalAlign::Superscript);
        assert_eq!(applied.highlight, Some(Color::rgb(255, 255, 0)));
        let before = app.editor.document().clone();
        call(
            &mut app,
            "folio_convert_case",
            json!({"selection":selection(0,7),"case":"upper"}),
        )
        .unwrap();
        assert_eq!(
            app.editor.document().paragraph(0).unwrap().runs[0].text,
            "STRASSE"
        );
        call(&mut app, "folio_undo", json!({})).unwrap();
        assert_eq!(app.editor.document(), &before);
        call(
            &mut app,
            "folio_format_text",
            json!({"selection":selection(0,7),"italic":true}),
        )
        .unwrap();
        assert!(
            app.editor.document().paragraph(0).unwrap().runs[0]
                .style
                .highlight
                .is_some()
        );
        call(
            &mut app,
            "folio_format_text",
            json!({"selection":selection(0,7),"highlight":null}),
        )
        .unwrap();
        assert!(
            app.editor.document().paragraph(0).unwrap().runs[0]
                .style
                .highlight
                .is_none()
        );
        for (kind, value) in [("multiple", 150), ("exact", 320), ("at_least", 400)] {
            call(&mut app,"folio_format_paragraph",json!({"selection":selection(0,0),"space_before_twips":120,"space_after_twips":240,"line_spacing":{"kind":kind,"value":value}})).unwrap();
            assert_eq!(
                document(&app)["blocks"][0]["style"]["line_spacing"],
                json!({"kind":kind,"value":value})
            );
        }
        let layout = json!({"size":{"width_twips":12240,"height_twips":15840},"orientation":"landscape","margins":{"top":720,"right":720,"bottom":720,"left":720}});
        call(&mut app, "folio_set_page_layout", json!({"layout":layout})).unwrap();
        assert_eq!(document(&app)["page_layout"], layout);
        let wire = document(&app);
        assert_eq!(
            wire["blocks"][0]["runs"][0]["style"]["vertical_align"],
            "superscript"
        );
        assert_eq!(wire["read_only"], false);
    }
    #[test]
    fn parity_invalid_inputs_are_transactional_and_nested_strict() {
        let mut app = FolioApp::default();
        call(
            &mut app,
            "folio_replace_text",
            json!({"selection":selection(0,0),"text":"Keep"}),
        )
        .unwrap();
        for (name, input) in [
            (
                "folio_format_text",
                json!({"selection":selection(0,4),"color":null}),
            ),
            (
                "folio_format_paragraph",
                json!({"selection":selection(0,0),"alignment":null}),
            ),
            (
                "folio_export_text",
                json!({"path":"/tmp/unused.txt","selection":null}),
            ),
            (
                "folio_format_text",
                json!({"selection":selection(0,4),"color":{"red":1,"green":2,"blue":3,"extra":true}}),
            ),
            (
                "folio_format_text",
                json!({"selection":selection(0,4),"highlight":{"red":256,"green":0,"blue":0}}),
            ),
            (
                "folio_format_text",
                json!({"selection":selection(0,4),"vertical_align":"over"}),
            ),
            (
                "folio_format_paragraph",
                json!({"selection":selection(0,0),"line_spacing":{"kind":"multiple","value":65536}}),
            ),
            (
                "folio_format_paragraph",
                json!({"selection":selection(0,0),"line_spacing":{"kind":"exact","value":0}}),
            ),
            (
                "folio_format_paragraph",
                json!({"selection":selection(0,0),"line_spacing":{"kind":"exact","value":10,"extra":true}}),
            ),
            (
                "folio_set_page_layout",
                json!({"layout":{"size":{"width_twips":1,"height_twips":2},"orientation":"portrait","margins":{"top":0,"right":1,"bottom":0,"left":1}}}),
            ),
            (
                "folio_new_document",
                json!({"template":"unknown","discard_unsaved":true}),
            ),
        ] {
            let before = document(&app);
            app.typing = Some(TextStyle::default());
            assert!(call(&mut app, name, input).is_err(), "{name}");
            assert_eq!(document(&app), before, "{name}");
            assert!(app.typing.is_some());
        }
        call(&mut app, "folio_undo", json!({})).unwrap();
        assert_eq!(app.editor.document(), &Document::default());
    }
    #[test]
    fn parity_lifecycle_readonly_and_modal_guards() {
        let mut app = FolioApp::default();
        assert_eq!(
            call(&mut app, "folio_list_templates", json!({})).unwrap()["templates"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        app.read_only = true;
        call(
            &mut app,
            "folio_select",
            json!({"selection":selection(0,0)}),
        )
        .unwrap();
        assert!(call(&mut app, "folio_undo", json!({})).is_err());
        app.workbench.palette = true;
        assert!(call(&mut app, "folio_new_document", json!({})).is_err());
        assert!(
            call(
                &mut app,
                "folio_select",
                json!({"selection":selection(0,0)})
            )
            .is_err()
        );
        assert!(call(&mut app, "folio_get_document", json!({})).is_ok());
        app.workbench.palette = false;
        call(&mut app, "folio_new_document", json!({"template":"letter"})).unwrap();
        assert!(app.editor.is_dirty());
        assert!(!app.read_only);
        call(
            &mut app,
            "folio_new_document",
            json!({"discard_unsaved":true}),
        )
        .unwrap();
        assert!(!app.editor.is_dirty());
    }
    #[test]
    fn copy_export_reject_relative_and_empty_paths_without_side_effects() {
        let relative_dir = PathBuf::from(format!(
            ".folio-mcp-path-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&relative_dir).unwrap();
        struct OwnedDir(PathBuf);
        impl Drop for OwnedDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _owned = OwnedDir(relative_dir.clone());
        let mut app = FolioApp::default();
        call(
            &mut app,
            "folio_replace_text",
            json!({"selection":selection(0,0),"text":"Keep"}),
        )
        .unwrap();
        app.typing = Some(TextStyle::default());
        app.composition = Some("pending".into());
        app.ime_enabled = true;
        app.reveal = false;
        let before = document(&app);
        let mut failures = Vec::new();
        for (name, extension) in [
            ("folio_duplicate_document", "docx"),
            ("folio_export_text", "txt"),
        ] {
            for path in [
                relative_dir.join(format!("copy.{extension}")),
                PathBuf::new(),
                std::fs::canonicalize(&relative_dir)
                    .unwrap()
                    .join("wrong.bin"),
            ] {
                let result = call(&mut app, name, json!({"path":path,"overwrite":true}));
                if result.is_ok() {
                    failures.push(format!("{name}: {path:?}"));
                }
                assert_eq!(document(&app), before);
                assert_eq!(app.typing, Some(TextStyle::default()));
                assert_eq!(app.composition.as_deref(), Some("pending"));
                assert!(app.ime_enabled);
                assert!(!app.reveal);
            }
        }
        assert!(
            failures.is_empty(),
            "relative/empty paths accepted: {failures:?}"
        );
        assert_eq!(std::fs::read_dir(&relative_dir).unwrap().count(), 0);
        call(&mut app, "folio_undo", json!({})).unwrap();
        assert_eq!(app.editor.document(), &Document::default());
    }
    #[test]
    fn parity_copy_export_readonly_save_preserve_state_and_protect_source() {
        let dir = std::env::temp_dir().join(format!(
            "folio-mcp-parity-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        struct OwnedDir(PathBuf);
        impl Drop for OwnedDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _owned = OwnedDir(dir.clone());
        let source = dir.join("source.docx");
        let copy = dir.join("copy.docx");
        let text = dir.join("out.txt");
        let mut app = FolioApp::default();
        call(
            &mut app,
            "folio_replace_text",
            json!({"selection":selection(0,0),"text":"Café\nNext"}),
        )
        .unwrap();
        call(&mut app, "folio_save_document", json!({"path":source})).unwrap();
        app.read_only = true;
        let before = document(&app);
        call(&mut app, "folio_duplicate_document", json!({"path":copy})).unwrap();
        call(
            &mut app,
            "folio_export_text",
            json!({"path":text,"selection":selection(5,0)}),
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&text).unwrap(), "Café");
        assert_eq!(document(&app), before);
        assert!(call(&mut app, "folio_duplicate_document", json!({"path":copy})).is_err());
        assert!(call(&mut app, "folio_export_text", json!({"path":text})).is_err());
        call(
            &mut app,
            "folio_export_text",
            json!({"path":text,"overwrite":true}),
        )
        .unwrap();
        assert_eq!(std::fs::read_to_string(&text).unwrap(), "Café\nNext");
        for name in ["folio_save_document", "folio_duplicate_document"] {
            assert!(call(&mut app, name, json!({"path":source,"overwrite":true})).is_err());
            assert_eq!(document(&app), before);
        }
        assert!(
            call(
                &mut app,
                "folio_export_text",
                json!({"path":dir.join("failed.txt"),"selection":selection(1,4)})
            )
            .is_err()
        );
        assert!(
            call(
                &mut app,
                "folio_duplicate_document",
                json!({"path":dir.join("missing/copy.docx")})
            )
            .is_err()
        );
        assert_eq!(document(&app), before);
        for (name, input) in [
            (
                "folio_replace_text",
                json!({"selection":selection(0,0),"text":"bad"}),
            ),
            (
                "folio_format_text",
                json!({"selection":selection(0,5),"bold":true}),
            ),
            (
                "folio_format_paragraph",
                json!({"selection":selection(0,0),"alignment":"right"}),
            ),
            (
                "folio_convert_case",
                json!({"selection":selection(0,5),"case":"upper"}),
            ),
            (
                "folio_replace_all",
                json!({"text":"Café","replacement":"bad"}),
            ),
            (
                "folio_insert_page_break",
                json!({"selection":selection(0,0)}),
            ),
            ("folio_undo", json!({})),
            ("folio_redo", json!({})),
        ] {
            assert!(call(&mut app, name, input).is_err());
            assert_eq!(document(&app), before);
        }
        call(
            &mut app,
            "folio_save_document",
            json!({"path":dir.join("save-as.docx")}),
        )
        .unwrap();
        assert!(app.read_only);
        call(&mut app, "folio_open_document", json!({"path":copy})).unwrap();
        assert!(!app.read_only);
        call(&mut app, "folio_undo", json!({})).unwrap();
        assert_eq!(app.editor.document().paragraph(0).unwrap().text(), "Café");
    }
    #[test]
    fn parity_nested_layout_schema_and_invalid_history() {
        let schemas = tools();
        let schema =
            |name: &str| &schemas.iter().find(|tool| tool["name"] == name).unwrap()["inputSchema"];
        assert_eq!(
            schema("folio_find")["properties"]["match_case"]["default"],
            true
        );
        assert_eq!(
            schema("folio_find")["properties"]["whole_words"]["default"],
            false
        );
        assert_eq!(
            schema("folio_format_paragraph")["required"],
            json!(["selection"])
        );
        let mut app = FolioApp::default();
        call(
            &mut app,
            "folio_replace_text",
            json!({"selection":selection(0,0),"text":"First"}),
        )
        .unwrap();
        call(
            &mut app,
            "folio_replace_text",
            json!({"selection":selection(5,5),"text":" next"}),
        )
        .unwrap();
        call(&mut app, "folio_undo", json!({})).unwrap();
        let before = document(&app);
        let layout = json!({"size":{"width_twips":12240,"height_twips":15840},"orientation":"portrait","margins":{"top":720,"right":720,"bottom":720,"left":720}});
        for location in ["layout", "size", "margins"] {
            let mut bad = layout.clone();
            if location == "layout" {
                bad["extra"] = json!(true);
            } else {
                bad[location]["extra"] = json!(true);
            }
            assert!(call(&mut app, "folio_set_page_layout", json!({"layout":bad})).is_err());
            assert_eq!(document(&app), before);
        }
        for (name, bad) in [
            ("folio_find", json!({"text":"First","match_case":null})),
            (
                "folio_convert_case",
                json!({"selection":selection(0,5),"case":"invalid"}),
            ),
            (
                "folio_format_text",
                json!({"selection":{"anchor":{"block":0,"offset":0,"extra":true},"focus":{"block":0,"offset":5}},"bold":true}),
            ),
            (
                "folio_format_text",
                json!({"selection":selection(0,5),"highlight":{"red":0,"green":0,"blue":0,"extra":true}}),
            ),
        ] {
            assert!(call(&mut app, name, bad).is_err());
            assert_eq!(document(&app), before);
        }
        call(&mut app, "folio_redo", json!({})).unwrap();
        assert_eq!(
            app.editor.document().paragraph(0).unwrap().text(),
            "First next"
        );
        call(&mut app, "folio_set_page_layout", json!({"layout":layout})).unwrap();
        call(&mut app, "folio_undo", json!({})).unwrap();
        assert_eq!(app.editor.document().page_layout, PageLayout::default());
    }
    #[test]
    fn edits_are_unicode_safe_atomic_and_undoable() {
        let mut app = FolioApp::default();
        call(
            &mut app,
            "folio_replace_text",
            json!({"selection":selection(0,0),"text":"Café 👩‍💻"}),
        )
        .unwrap();
        let before = app.editor.document().clone();
        assert!(
            call(
                &mut app,
                "folio_replace_text",
                json!({"selection":selection(4,5),"text":"X"})
            )
            .is_err()
        );
        assert_eq!(app.editor.document(), &before);
        assert!(
            call(
                &mut app,
                "folio_replace_text",
                json!({"selection":selection(0,5),"text":"Tea","expected_text":"Coffee"})
            )
            .is_err()
        );
        assert_eq!(app.editor.document(), &before);
        call(
            &mut app,
            "folio_format_text",
            json!({"selection":selection(0,5),"bold":true}),
        )
        .unwrap();
        assert!(
            app.editor.document().paragraph(0).unwrap().runs[0]
                .style
                .bold
        );
        call(&mut app, "folio_undo", json!({})).unwrap();
        assert_eq!(app.editor.document(), &before);
        call(&mut app, "folio_redo", json!({})).unwrap();
        assert!(
            app.editor.document().paragraph(0).unwrap().runs[0]
                .style
                .bold
        );
    }
    #[test]
    fn protects_unsaved_work_and_invalid_arguments() {
        let mut app = FolioApp::default();
        call(
            &mut app,
            "folio_replace_text",
            json!({"selection":selection(0,0),"text":"Keep"}),
        )
        .unwrap();
        assert!(call(&mut app, "folio_new_document", json!({})).is_err());
        assert!(
            call(
                &mut app,
                "folio_open_document",
                json!({"path":"relative.docx"})
            )
            .is_err()
        );
        assert!(
            call(
                &mut app,
                "folio_select",
                json!({"selection":selection(0,100)})
            )
            .is_err()
        );
        assert!(call(&mut app, "folio_undo", json!({"unexpected":true})).is_err());
        assert_eq!(app.editor.document().paragraph(0).unwrap().text(), "Keep");
        call(
            &mut app,
            "folio_new_document",
            json!({"discard_unsaved":true}),
        )
        .unwrap();
        assert!(!app.editor.is_dirty());
    }
    #[test]
    fn stdio_handshake_discovery_notifications_and_errors() {
        let messages=[
            json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"folio_get_document"}}),
            json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"unknown"}}),
        ].into_iter().map(|v|format!("{v}\n")).collect::<String>();
        let mut output = Vec::new();
        let mut app = FolioApp::default();
        serve(&mut io::Cursor::new(messages), &mut output, |name, args| {
            call(&mut app, name, args)
        })
        .unwrap();
        let replies: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(replies.len(), 5);
        assert_eq!(replies[0]["error"]["code"], -32000);
        assert_eq!(replies[1]["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(replies[2]["result"]["tools"].as_array().unwrap().len(), 19);
        assert_eq!(replies[3]["result"]["structuredContent"]["dirty"], false);
        assert_eq!(replies[4]["error"]["code"], -32602);
    }
    #[test]
    fn modern_discovery_and_inline_requests() {
        let mut protocol = Protocol::default();
        let mut app = FolioApp::default();
        let meta = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}});
        let result = protocol
            .handle(
                json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":meta}}),
                &mut |n, a| call(&mut app, n, a),
            )
            .unwrap();
        assert_eq!(result["result"]["supportedVersions"][0], "2026-07-28");
        assert_eq!(result["result"]["resultType"], "complete");
        let result=protocol.handle(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"_meta":meta,"name":"folio_get_document"}}),&mut |n,a|call(&mut app,n,a)).unwrap();
        assert_eq!(result["result"]["isError"], false);
    }
    #[test]
    fn malformed_input_does_not_break_next_request() {
        let mut output = Vec::new();
        serve(
            &mut io::Cursor::new(b"not json\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n"),
            &mut output,
            |_, _| unreachable!(),
        )
        .unwrap();
        let replies: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(replies[0]["error"]["code"], -32700);
        assert_eq!(replies[1]["result"], json!({}));
    }

    #[test]
    fn save_open_and_source_protection_use_existing_file_guards() {
        let dir = std::env::temp_dir().join(format!(
            "folio-mcp-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("document.docx");
        let mut app = FolioApp::default();
        call(
            &mut app,
            "folio_replace_text",
            json!({"selection":selection(0,0),"text":"MCP document"}),
        )
        .unwrap();
        call(&mut app, "folio_save_document", json!({"path":path})).unwrap();
        assert!(!app.editor.is_dirty());
        let bytes = std::fs::read(&path).unwrap();
        assert!(call(&mut app, "folio_save_document", json!({"path":path})).is_err());
        let mut reopened = FolioApp::default();
        call(&mut reopened, "folio_open_document", json!({"path":path})).unwrap();
        assert_eq!(
            reopened.editor.document().paragraph(0).unwrap().text(),
            "MCP document"
        );
        app.protected = Some(path.clone());
        let alias = dir.join("alias.docx");
        std::fs::hard_link(&path, &alias).unwrap();
        assert!(
            call(
                &mut app,
                "folio_save_document",
                json!({"path":alias,"overwrite":true})
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn live_bridge_authenticates_updates_visible_editor_and_revokes_access() {
        let ctx = egui::Context::default();
        let bridge = Bridge::start(ctx.clone()).unwrap();
        let address = bridge.address;
        let mut bad = TcpStream::connect(address).unwrap();
        bad.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        write_message(&mut bad, &json!({"token":"wrong"})).unwrap();
        let mut byte = [0];
        assert!(matches!(bad.read(&mut byte), Ok(0) | Err(_)));
        assert!(bridge.receiver.try_recv().is_err());
        let token = bridge.token.clone();
        let (client_ready, ready) = mpsc::channel();
        let (revoke, revoked) = mpsc::channel();
        let client = thread::spawn(move || {
            let mut socket = TcpStream::connect(address).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut input = BufReader::new(socket.try_clone().unwrap());
            write_message(&mut socket, &json!({"token":token})).unwrap();
            let auth: Value =
                serde_json::from_slice(&read_message(&mut input).unwrap().unwrap()).unwrap();
            assert_eq!(auth["authenticated"], true);
            let meta = json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}});
            write_message(&mut socket,&json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"_meta":meta,"name":"folio_replace_text","arguments":{"selection":selection(0,0),"text":"Visible AI edit"}}})).unwrap();
            let result: Value =
                serde_json::from_slice(&read_message(&mut input).unwrap().unwrap()).unwrap();
            assert_eq!(result["result"]["isError"], false);
            client_ready.send(()).unwrap();
            revoked.recv_timeout(Duration::from_secs(3)).unwrap();
            assert!(matches!(read_message(&mut input), Ok(None) | Err(_)));
        });
        let mut app = FolioApp::default();
        let request = bridge
            .receiver
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        request
            .reply
            .send(call(&mut app, &request.name, request.args))
            .unwrap();
        ready.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(
            app.editor.document().paragraph(0).unwrap().text(),
            "Visible AI edit"
        );
        assert!(app.editor.can_undo());
        // A background session owns an independent editor.
        assert_eq!(
            FolioApp::default()
                .editor
                .document()
                .paragraph(0)
                .unwrap()
                .text(),
            ""
        );
        drop(bridge);
        revoke.send(()).unwrap();
        client.join().unwrap();
    }
}
