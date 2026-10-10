# Folio MCP interface

Folio exposes 19 document tools through the Model Context Protocol. Any AI
provider can use them through a client that supports local MCP servers over
stdio. A model by itself needs a tool-calling host; Folio does not require an
AI API key or send documents to a provider on its own.

## Live document

1. Open Folio and choose **View → AI connection**.
2. Choose **Enable live access**.
3. Choose **Copy MCP configuration** and add it to your client's MCP settings.
4. Start the server in that client. Its tools operate on this window's current
   document, including unsaved edits. Changes appear in Folio and share the
   normal undo/redo history.

The copied configuration runs the Folio executable with
`--mcp-connect ADDRESS` and passes `FOLIO_MCP_TOKEN` in the server environment,
keeping the token out of command-line arguments. A random session token authenticates a private
loopback connection. This bridge is not an HTTP MCP endpoint; the executable
adapts it to standard MCP stdio for the client. Each window has its own address
and token. Access starts disabled. **Disable live access** or quit Folio to close
connections and revoke the token. Enabling access again generates a new
configuration. The token is not saved to a file by Folio; treat the copied
configuration as a credential for that session.
An access indicator in the status bar (or focus bar) opens these settings while
live access is enabled.

Up to eight clients can connect to a window. Requests execute sequentially on
the UI thread. Open recovery, unsaved-change, overwrite, or error dialogs block tool calls
until resolved. Workbench dialogs (including the template gallery) block actions
that change selection, edit, switch documents or write files; read tools remain available. Closing the AI connection settings window does not disable access.

## Background documents

Configure a separate server with the same executable and `--mcp`:

```json
{
  "mcpServers": {
    "folio-background": {
      "command": "/absolute/path/to/folio-desktop",
      "args": ["--mcp"]
    }
  }
}
```

On macOS the packaged command is
`/absolute/path/to/Folio.app/Contents/MacOS/folio-desktop`.
The AI connection window can copy a background configuration with the actual path.

Each background process owns an independent, initially blank document, selection,
and undo history. No GUI or running Folio window is needed. Use
`folio_open_document` to load a DOCX and `folio_save_document` to save it.
Unsaved background content is lost when the client ends the process; save explicitly.
You can configure live and background servers together with distinct names.

## Tools

| Tool | Function |
| --- | --- |
| `folio_get_document` | Read indexed blocks, text/runs/full styles, paragraph styles, page layout, read-only mode, file path, warnings, statistics, dirty state, and undo/redo availability |
| `folio_get_selection` | Read selection endpoints and selected text |
| `folio_select` | Set explicit selection endpoints |
| `folio_replace_text` | Insert at a caret, replace a range, or delete by supplying empty text |
| `folio_format_text` | Patch bold, italic, underline, strikethrough, vertical alignment, color, highlight, font family, or font size on a nonempty range |
| `folio_format_paragraph` | Patch optional alignment, paragraph spacing, and line spacing |
| `folio_find` | Return ranges for paragraph-local literal matches with optional case and whole-word controls |
| `folio_replace_all` | Replace matches as one undoable operation |
| `folio_insert_page_break` | Replace a range or insert at a caret with a page break |
| `folio_undo` | Undo the latest document edit |
| `folio_redo` | Redo the latest undone edit |
| `folio_new_document` | Create a blank document or a named template |
| `folio_open_document` | Import an absolute DOCX path and report warnings |
| `folio_save_document` | Atomically save an absolute DOCX path |
| `folio_convert_case` | Apply Unicode upper, lower, title, or sentence case while preserving structure and styles |
| `folio_set_page_layout` | Set complete nominal page size, orientation and margins |
| `folio_list_templates` | Read stable template IDs, names and descriptions |
| `folio_duplicate_document` | Write a distinct DOCX copy without changing the active editor |
| `folio_export_text` | Export whole document or optional selection to a distinct UTF-8 TXT file |

Discover the full argument schemas using `tools/list`. All editing ranges use:

```json
{
  "selection": {
    "anchor": {"block": 0, "offset": 0},
    "focus": {"block": 0, "offset": 5}
  }
}
```

Block indices and offsets are zero-based. Offsets count UTF-8 **bytes**, and must
fall at Unicode grapheme boundaries. Read the indexed blocks before editing;
do not count characters as bytes. Equal endpoints represent a caret. Newlines
in inserted text create paragraphs. Font sizes use half-points (`24` is 12 pt).
Optional `expected_text` on `folio_replace_text` rejects a range whose contents
have changed since it was read, which is useful while a person is also editing.

`folio_find` and `folio_replace_all` accept optional `match_case` (default true)
and `whole_words` (default false). Search uses Unicode word boundaries and
returns grapheme-safe byte ranges within paragraphs, including matches across runs.
It does not match across paragraph or page boundaries. Case conversion uses
`case: "upper" | "lower" | "title" | "sentence"` and a nonempty selection.
Mappings are locale independent and may change byte lengths; use returned selection
endpoints. Title capitalizes whitespace-delimited tokens; sentence capitalization
follows punctuation and whitespace.

Text `color` and `highlight` use integer RGB channels:
`{"red":255,"green":255,"blue":0}`. Missing highlight preserves it, while
`"highlight": null` clears it. `vertical_align` is `baseline`, `superscript`, or
`subscript`. Every nested input rejects unknown fields; explicit null is rejected
for fields other than highlight. Invalid arguments leave selection, document and
undo/redo history unchanged. DOCX preserves standard palette highlights with `w:highlight` and arbitrary RGB
highlight values with clear `w:shd` run shading.

Paragraph `alignment` is optional so spacing-only patches work.
`space_before_twips` and `space_after_twips` are nonnegative integers.
`line_spacing` uses `{"kind":"multiple","value":150}` for 150 percent;
`exact` and `at_least` use positive twips. Multiple percentages are 1–65535;
other paragraph distances are u32 values. One twip is 1/1440 inch.
`folio_get_document` preserves the existing paragraph-level `alignment` and adds
complete paragraph `style` plus full run and `default_style` properties.

Page layout input is complete rather than a patch; read output returns the same
shape in `page_layout`:

```json
{"layout":{"size":{"width_twips":12240,"height_twips":15840},
 "orientation":"landscape","margins":{"top":720,"right":720,"bottom":720,"left":720}}}
```

Size is nominal before orientation; landscape swaps the effective dimensions.
Margins must leave positive content area. The core validates page layouts and
all edits use the normal shared undo history.

`folio_list_templates` returns `templates` entries with `id`, `name`, and
`description`. Optional `template` on `folio_new_document` accepts `blank`,
`letter`, `meeting_notes`, or `project_brief`; omission creates Blank. Nonblank
templates start dirty. Duplicate accepts absolute `.docx` `path`; text export
accepts absolute `.txt` `path` and optional `selection`. Both default
`overwrite` to false, reject active-source aliases, and preserve active document,
selection, history, path, import warnings and dirty state. Text export separates
paragraphs with newlines and page breaks with form feeds.

Read output includes `read_only`. Read-only mode blocks text/format/page edits
and undo/redo, while navigation, reads, export, duplicate and distinct Save As
remain available. Saving over the active source or its aliases is blocked.
New/Open remain available and leave read-only mode after a successful switch.
There is no MCP tool to disable the UI's read-only mode.

New/open refuse to discard dirty documents unless `discard_unsaved: true` is
explicitly supplied. Save refuses an existing destination unless `overwrite: true`
is supplied. Warned import sources and their aliases cannot be overwritten, even
with that flag. DOCX compatibility remains the same bounded subset as the GUI.
File tools can access paths permitted to the Folio process; stdio hosts should
apply their normal tool approvals and filesystem restrictions.

## Protocol and verification

The server supports MCP `2026-07-28` discovery and per-request metadata, plus the
initialization lifecycle of `2025-11-25`, `2025-06-18`, `2025-03-26`, and
`2024-11-05`. Transport is newline-delimited JSON-RPC over stdio; diagnostics use
stderr. Input messages are limited to 4 MiB. Resources, prompts, streaming HTTP,
sampling, and asynchronous tasks are not advertised.

Workspace tests cover protocol lifecycles, schema/argument failures, Unicode-safe
atomic edits, undo/redo, save/open protection, live authentication, and revocation.
An optional interoperability test uses the official Python MCP SDK:

```sh
uv run --with mcp==2.3.0 python scripts/test-mcp.py /absolute/path/to/folio-desktop
```

To test a live blank window, append its copied `--mcp-connect ADDRESS` arguments
and set `FOLIO_MCP_TOKEN` from the configuration's `env` object. The live script requires a clean blank writable test window, inserts sample text,
finds it, and undoes the edit. It never saves or exports live documents. Background
mode also exercises templates, case/search/formatting/page layout, copy/text export
and DOCX save/reopen using disposable documents and temporary paths. Append
`--offline` to `uv run` when the pinned SDK is already cached.

Protocol references: [MCP stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports),
[tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools), and
[2026-07-28 schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/schema/2026-07-28/schema.ts).
