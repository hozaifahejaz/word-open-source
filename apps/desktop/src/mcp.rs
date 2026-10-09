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
    let definitions = vec![
        (
            "folio_get_document",
            "Read document blocks, rich text styles, statistics, file path and dirty state.",
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
            "Format an explicit nonempty selection; font sizes are half-points (24 = 12pt).",
            object(
                json!({"selection":selection,"bold":boolean,"italic":boolean,"underline":boolean,"font_family":string,"size_half_points":{"type":"integer","minimum":1,"maximum":65535}}),
                &["selection"],
            ),
            false,
        ),
        (
            "folio_format_paragraph",
            "Set paragraph alignment for an explicit selection.",
            object(
                json!({"selection":selection,"alignment":{"type":"string","enum":["left","center","right","justify"]}}),
                &["selection", "alignment"],
            ),
            false,
        ),
        (
            "folio_find",
            "Find literal, case-sensitive text within paragraphs; return selection ranges.",
            object(json!({"text":string}), &["text"]),
            true,
        ),
        (
            "folio_replace_all",
            "Replace all literal matches as one undoable edit.",
            object(
                json!({"text":string,"replacement":string}),
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
            "Start a blank document. Refuses unsaved changes unless discard_unsaved is explicitly true.",
            object(json!({"discard_unsaved":boolean}), &[]),
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
            "Atomically save to an absolute DOCX path. Existing files require overwrite=true. Warned import sources remain protected.",
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
    expected_text: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Format {
    selection: Range,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    font_family: Option<String>,
    size_half_points: Option<u16>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ParagraphFormat {
    selection: Range,
    alignment: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplaceAll {
    text: String,
    replacement: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct New {
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

fn args<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid arguments: {e}"))
}
fn range(selection: Selection) -> Value {
    json!({"anchor":{"block":selection.anchor.block,"offset":selection.anchor.offset},"focus":{"block":selection.focus.block,"offset":selection.focus.offset}})
}
fn style(s: &TextStyle) -> Value {
    json!({"bold":s.bold,"italic":s.italic,"underline":s.underline,"font_family":s.font_family,"size_half_points":s.size_half_points,"color":{"red":s.color.red,"green":s.color.green,"blue":s.color.blue}})
}
fn document(app: &FolioApp) -> Value {
    let blocks: Vec<_> = app.editor.document().blocks.iter().enumerate().map(|(index, b)| match b {
        Block::PageBreak => json!({"index":index,"type":"page_break"}),
        Block::Paragraph(p) => json!({"index":index,"type":"paragraph","text":p.text(),"byte_length":p.len_bytes(),"alignment":format!("{:?}",p.style.alignment).to_lowercase(),"default_style":style(&p.default_style),"runs":p.runs.iter().map(|r|json!({"text":r.text,"style":style(&r.style)})).collect::<Vec<_>>()})
    }).collect();
    let stats = editing::document_statistics(app.editor.document());
    json!({"blocks":blocks,"selection":range(app.editor.selection()),"path":app.path.as_ref().map(|p|p.to_string_lossy()),"dirty":app.editor.is_dirty(),"can_undo":app.editor.can_undo(),"can_redo":app.editor.can_redo(),"statistics":{"words":stats.words,"characters":stats.characters},"warnings":app.warnings.iter().map(|w|&w.message).collect::<Vec<_>>()})
}
fn check_path(path: &std::path::Path) -> Result<(), String> {
    if !path.is_absolute()
        || !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("docx"))
    {
        return Err("Use an absolute .docx path".into());
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
    if app.pending.is_some() || app.overwrite.is_some() || app.error.is_some() {
        return Err("Resolve the open Folio dialog before using MCP tools".into());
    }
    let command = match name {
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
                .find(&a.text)
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
                    ..Default::default()
                },
            }
        }
        "folio_format_paragraph" => {
            let a: ParagraphFormat = args(arguments)?;
            let alignment = match a.alignment.as_str() {
                "left" => Alignment::Left,
                "center" => Alignment::Center,
                "right" => Alignment::Right,
                "justify" => Alignment::Justify,
                _ => return Err("Unknown alignment".into()),
            };
            Command::FormatParagraphs {
                selection: a.selection.selection(),
                patch: ParagraphPatch {
                    alignment: Some(alignment),
                    ..Default::default()
                },
            }
        }
        "folio_replace_all" => {
            let a: ReplaceAll = args(arguments)?;
            Command::ReplaceAll {
                needle: a.text,
                replacement: a.replacement,
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
            app.new_document()?;
            reset_edit_state(app);
            return Ok(document(app));
        }
        "folio_open_document" => {
            let a: Open = args(arguments)?;
            check_path(&a.path)?;
            check_unsaved(app, a.discard_unsaved)?;
            app.open_document(a.path)?;
            reset_edit_state(app);
            return Ok(document(app));
        }
        "folio_save_document" => {
            let a: Save = args(arguments)?;
            check_path(&a.path)?;
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
        assert_eq!(replies[2]["result"]["tools"].as_array().unwrap().len(), 14);
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
