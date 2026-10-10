#!/usr/bin/env python3
"""Official Python MCP SDK interoperability smoke.

Run: uv run --offline --with mcp==2.3.0 python scripts/test-mcp.py /absolute/path/to/folio-desktop
Background mode owns disposable documents and temporary save/export destinations.
For live mode append --mcp-connect ADDRESS and set FOLIO_MCP_TOKEN. Only use a
clean blank test window: live smoke inserts, finds and undoes text; it never saves.
"""
import os
from pathlib import Path
import sys
import tempfile
import zipfile

import anyio
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

TOOLS = {
    "folio_get_document", "folio_get_selection", "folio_select", "folio_replace_text",
    "folio_format_text", "folio_format_paragraph", "folio_find", "folio_replace_all",
    "folio_insert_page_break", "folio_undo", "folio_redo", "folio_new_document",
    "folio_open_document", "folio_save_document", "folio_convert_case",
    "folio_set_page_layout", "folio_list_templates", "folio_duplicate_document",
    "folio_export_text", "folio_export_document",
}


def selection(start, end, block=0):
    return {"anchor": {"block": block, "offset": start},
            "focus": {"block": block, "offset": end}}


async def checked(session, name, arguments=None):
    result = await session.call_tool(name, arguments or {})
    assert not result.is_error, (name, result.content)
    assert result.structured_content is not None, name
    return result.structured_content


async def background_smoke(session):
    catalog = await checked(session, "folio_list_templates")
    assert [entry["id"] for entry in catalog["templates"]] == [
        "blank", "letter", "meeting_notes", "project_brief"]
    template = await checked(session, "folio_new_document", {"template": "letter"})
    assert template["dirty"] and template["blocks"][0]["text"] == "A personal letter"
    await checked(session, "folio_new_document", {"discard_unsaved": True})
    text = "Straße CAT cat cats\nSecond paragraph"
    await checked(session, "folio_replace_text", {"selection": selection(0, 0), "text": text})
    matches = await checked(session, "folio_find", {"text": "cat", "match_case": False, "whole_words": True})
    assert len(matches["matches"]) == 2
    replaced = await checked(session, "folio_replace_all", {
        "text": "cat", "replacement": "dog", "match_case": False, "whole_words": True})
    assert replaced["replacements"] == 2
    await checked(session, "folio_undo")
    rgb = {"red": 255, "green": 255, "blue": 0}
    await checked(session, "folio_format_text", {
        "selection": selection(0, 7), "bold": True, "italic": True,
        "underline": True, "strikethrough": True, "vertical_align": "superscript",
        "color": {"red": 18, "green": 52, "blue": 86}, "highlight": rgb,
        "font_family": "Arial", "size_half_points": 28})
    await checked(session, "folio_format_text", {"selection": selection(0, 7), "highlight": None})
    cleared = await checked(session, "folio_get_document")
    assert cleared["blocks"][0]["runs"][0]["style"]["highlight"] is None
    await checked(session, "folio_undo")
    await checked(session, "folio_convert_case", {"selection": selection(0, 7), "case": "upper"})
    cased = await checked(session, "folio_get_document")
    assert cased["blocks"][0]["runs"][0]["text"] == "STRASSE"
    await checked(session, "folio_undo")
    for kind, value in [("multiple", 150), ("exact", 360), ("at_least", 400)]:
        await checked(session, "folio_format_paragraph", {
            "selection": selection(0, 0), "space_before_twips": 120,
            "space_after_twips": 240, "line_spacing": {"kind": kind, "value": value}})
    layout = {"size": {"width_twips": 12240, "height_twips": 15840},
              "orientation": "landscape",
              "margins": {"top": 720, "right": 720, "bottom": 720, "left": 720}}
    await checked(session, "folio_set_page_layout", {"layout": layout})
    with tempfile.TemporaryDirectory(prefix="folio-mcp-") as directory:
        directory = Path(directory)
        saved, duplicate, exported = (directory / name for name in
                                      ["roundtrip.docx", "duplicate.docx", "export.txt"])
        await checked(session, "folio_save_document", {"path": str(saved)})
        expected = await checked(session, "folio_get_document")
        assert expected["page_layout"] == layout
        await checked(session, "folio_duplicate_document", {"path": str(duplicate)})
        await checked(session, "folio_export_text", {"path": str(exported)})
        assert exported.read_text() == text
        refused = await session.call_tool("folio_duplicate_document", {"path": str(duplicate)})
        assert refused.is_error
        after = await checked(session, "folio_get_document")
        assert after == expected
        for format_name, extension in [("pdf", "pdf"), ("docx", "docx"), ("odt", "odt"), ("rtf", "rtf"), ("html", "html"), ("markdown", "md"), ("txt", "txt")]:
            destination = directory / ("full-export." + extension)
            await checked(session, "folio_export_document", {"path": str(destination), "format": format_name})
            payload = destination.read_bytes()
            assert payload
            if format_name == "pdf":
                assert payload.startswith(b"%PDF-")
            elif format_name in {"docx", "odt"}:
                with zipfile.ZipFile(destination) as archive:
                    assert ("word/document.xml" if format_name == "docx" else "content.xml") in archive.namelist()
                    if format_name == "odt":
                        assert archive.read("mimetype") == b"application/vnd.oasis.opendocument.text"
            elif format_name == "rtf":
                assert payload.startswith(b"{\\rtf1")
            elif format_name == "html":
                assert b"<html" in payload.lower()
            else:
                assert "Straße" in payload.decode("utf-8")
            assert await checked(session, "folio_get_document") == expected
        for arguments in [{"path": "relative.pdf", "format": "pdf"}, {"path": str(directory / "unknown.pdf"), "format": "unknown"}]:
            assert (await session.call_tool("folio_export_document", arguments)).is_error
            assert await checked(session, "folio_get_document") == expected
        await checked(session, "folio_new_document")
        reopened = await checked(session, "folio_open_document", {"path": str(saved)})
        assert reopened["page_layout"] == expected["page_layout"]
        assert reopened["blocks"] == expected["blocks"]
        assert not reopened["dirty"]
        await checked(session, "folio_open_document", {"path": str(duplicate)})
        duplicate_read = await checked(session, "folio_get_document")
        assert duplicate_read["blocks"] == expected["blocks"]


async def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    arguments = sys.argv[2:] or ["--mcp"]
    live = "--mcp-connect" in arguments
    if not live and arguments != ["--mcp"]:
        raise SystemExit("Use --mcp or --mcp-connect ADDRESS")
    parameters = StdioServerParameters(
        command=sys.argv[1], args=arguments,
        env={"FOLIO_MCP_TOKEN": os.environ.get("FOLIO_MCP_TOKEN", "")},
    )
    with anyio.fail_after(60):
        async with stdio_client(parameters) as (reader, writer):
            async with ClientSession(reader, writer) as session:
                initialized = await session.initialize()
                assert initialized.server_info.name == "folio"
                tools = await session.list_tools()
                assert {tool.name for tool in tools.tools} == TOOLS
                before = await checked(session, "folio_get_document")
                assert (len(before["blocks"]) == 1 and before["blocks"][0]["type"] == "paragraph"
                        and before["blocks"][0]["text"] == "" and not before["dirty"]
                        and not before["read_only"]), "Smoke requires a clean blank writable document"
                await checked(session, "folio_replace_text", {
                    "selection": selection(0, 0), "text": "Hello from MCP — Café 👩‍💻", "expected_text": ""})
                current = await checked(session, "folio_get_document")
                assert current["blocks"][0]["text"].startswith("Hello from MCP")
                found = await checked(session, "folio_find", {"text": "Café"})
                assert len(found["matches"]) == 1
                await checked(session, "folio_undo")
                restored = await checked(session, "folio_get_document")
                assert restored["blocks"] == before["blocks"]
                assert restored["dirty"] == before["dirty"]
                if not live:
                    await background_smoke(session)
    print("Official MCP SDK: 20 tools, read/edit/find/undo PASS; " +
          ("live fixture restored" if live else "background parity and DOCX save/reopen PASS"))


if __name__ == "__main__":
    anyio.run(main)
