#!/usr/bin/env python3
"""Optional interoperability smoke test using the official Python MCP SDK.

Run: uv run --with mcp==2.3.0 python scripts/test-mcp.py /absolute/path/to/folio-desktop
Pass --mcp-connect ADDRESS after the executable and set FOLIO_MCP_TOKEN to test a live window.
Use a blank test document for live mode; this script makes an undoable edit.
"""
import os
import sys

import anyio
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client


async def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    parameters = StdioServerParameters(
        command=sys.argv[1], args=sys.argv[2:] or ["--mcp"],
        env={"FOLIO_MCP_TOKEN": os.environ.get("FOLIO_MCP_TOKEN", "")},
    )
    with anyio.fail_after(30):
        async with stdio_client(parameters) as (reader, writer):
            async with ClientSession(reader, writer) as session:
                initialized = await session.initialize()
                assert initialized.server_info.name == "folio"
                tools = await session.list_tools()
                assert len(tools.tools) == 14
                before = await session.call_tool("folio_get_document", {})
                assert not before.is_error
                assert before.structured_content["blocks"][0]["text"] == ""
                position = {"block": 0, "offset": 0}
                edited = await session.call_tool(
                    "folio_replace_text",
                    {"selection": {"anchor": position, "focus": position},
                     "text": "Hello from MCP — Café 👩‍💻", "expected_text": ""},
                )
                assert not edited.is_error
                current = await session.call_tool("folio_get_document", {})
                assert current.structured_content["blocks"][0]["text"].startswith("Hello from MCP")
                found = await session.call_tool("folio_find", {"text": "Café"})
                assert len(found.structured_content["matches"]) == 1
                undo = await session.call_tool("folio_undo", {})
                assert not undo.is_error
                restored = await session.call_tool("folio_get_document", {})
                assert restored.structured_content["blocks"] == before.structured_content["blocks"]
                assert restored.structured_content["dirty"] == before.structured_content["dirty"]
    print("Official MCP SDK: initialization, 14 tools, read/edit/find/undo PASS")


if __name__ == "__main__":
    anyio.run(main)
