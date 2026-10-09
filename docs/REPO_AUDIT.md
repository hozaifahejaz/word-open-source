# Repository audit — 2026-10-10

Source review covered document-core, DOCX import/export, desktop interaction,
layout, file operations, workspace/recovery, MCP, packaging, tests, documentation
and CI configuration. This records confirmed findings and release work; it is
not a guarantee that every defect has been found.

| Finding | Evidence and treatment |
| --- | --- |
| Recovery can resurrect undone edits | A dirty snapshot survived Undo to the clean baseline and ordinary Quit. Fixed in `0620fed`; regression covers idle/close cleanup, failed removal/retry and unreadable/future journal preservation. |
| Greek title case loses final-sigma context | `ΟΣ` became `Οσ`. Fixed in `0620fed` by retaining full-token lowercase context and scalar style ownership. Quoted title/sentence capitalization also improved with rich structure/undo regressions. |
| UI capabilities missing from MCP | Search options, structure-preserving case conversion, RGB, paragraph spacing and page geometry were inaccessible. Scheduled in the polished-editing plan, using shared commands. |
| Old process survives rebuild | Package replacement correctly preserves a running executable's inode. Release verification must quit and relaunch, and show a build revision to distinguish the new process. |
| Desktop guide is stale | Search/appearance instructions no longer match shipped features. Updating alongside the new workbench. |

Recovery Restore intentionally stays unsaved until explicit Save or another edit
schedules normal eligible-file autosave. It does not silently overwrite the file
being recovered. The regression suite now pins that policy.

The checkout-local toolchain is available; `cargo` not being in shell PATH does
not mean it is absent. `sh scripts/validate-local.sh` configures it and passed
the baseline formatting, tests, warning-free Clippy and Apple Silicon packaging.

Shipped functionality and limits are tracked in [FEATURES.md](FEATURES.md).
The [roadmap](superpowers/specs/2026-10-10-folio-feature-roadmap-design.md) still
includes substantial model/service work for tables, media, lists, references,
review, collaboration, encryption and printing. Those require real semantics,
rendering and codec support rather than inactive toolbar controls.
