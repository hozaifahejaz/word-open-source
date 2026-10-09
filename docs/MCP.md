# Folio MCP interface

Folio exposes 14 document tools through the Model Context Protocol. Any AI
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
the UI thread. Open unsaved-change, overwrite, or error dialogs block tool calls
until resolved. Closing the AI connection settings window does not disable access.

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
| `folio_get_document` | Read indexed blocks, text/runs/styles, file path, warnings, statistics, dirty state, and undo/redo availability |
| `folio_get_selection` | Read selection endpoints and selected text |
| `folio_select` | Set explicit selection endpoints |
| `folio_replace_text` | Insert at a caret, replace a range, or delete by supplying empty text |
| `folio_format_text` | Set bold, italic, underline, font family, or font size on a nonempty range |
| `folio_format_paragraph` | Set left, center, right, or justified alignment |
| `folio_find` | Return ranges for literal, case-sensitive matches within paragraphs |
| `folio_replace_all` | Replace matches as one undoable operation |
| `folio_insert_page_break` | Replace a range or insert at a caret with a page break |
| `folio_undo` | Undo the latest document edit |
| `folio_redo` | Redo the latest undone edit |
| `folio_new_document` | Create a blank document |
| `folio_open_document` | Import an absolute DOCX path and report warnings |
| `folio_save_document` | Atomically save an absolute DOCX path |

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
and set `FOLIO_MCP_TOKEN` from the configuration's `env` object. The script
inserts sample text, finds it, and undoes the edit.

Protocol references: [MCP stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports),
[tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools), and
[2026-07-28 schema](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/schema/2026-07-28/schema.ts).
