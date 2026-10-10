# folio-docx 0.1.0

A stream-based converter using `zip` and `quick-xml`, with no filesystem or
network operations. It uses the `document-core` model and established
`import_docx` / `export_docx` contracts.

## Supported conversion

Paragraphs and styled runs; bold, italic, single underline, strikethrough,
superscript/subscript and RGB text highlight; font family,
half-point size and RGB color; left/center/right/justified alignment;
before/after spacing in twips and multiple/exact/at-least line spacing;
explicit page breaks; and one document-wide size, orientation and margin set.
Matched ASCII/high-ANSI font slots map to one core font family. Font fallback,
East Asian/complex-script slots, theme fonts and theme colors cannot be preserved.

Import resolves document defaults, default/selected paragraph styles,
`basedOn` chains, character styles and direct formatting. Bold/italic/strike in style
chains use toggle semantics; direct formatting can explicitly clear them.
Direct paragraph-mark formatting controls the empty paragraph typing style and
does not incorrectly override existing runs. Paragraph-mark formatting nested in
named styles is omitted with a warning, requiring a converted copy.
Spacing attributes inherit independently.

Run highlights export as `w:shd w:val="clear"` with an RGB fill, allowing arbitrary
colors. Import accepts the 16 named `w:highlight` colors plus `none`, RGB clear
run shading, and solid shading only through its foreground `w:color` (never its
background `w:fill`). Unsupported patterns, automatic/missing shading colors and
theme shading warn. Import keeps highlight and shading layers through defaults,
named styles and direct formatting so highlight wins independent of XML child
order; clearing highlight reveals inherited shading. The public model retains
one visible background, so layered highlight/shading warns about its flattening.
These choices follow the official Open XML [Shading](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.shading?view=openxml-3.0.1)
and [Highlight](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.highlight?view=openxml-3.0.1)
semantics, reviewed 2026-10-10. Named styles remain flattened on export.
XML namespaces are resolved by URI, including alternate prefixes and the strict
WordprocessingML URI. Package relationships locate the main document and styles;
neither is assumed to live at a fixed path. UTF-8 XML, predefined/numeric entities,
Unicode, whitespace, tabs and XML CDATA are supported.

Export creates `[Content_Types].xml`, `_rels/.rels` and `word/document.xml`, with
valid main-document content type/relationship declarations. Paragraph and run
properties are explicit, so named styles are flattened into supported semantics.
Page dimensions are oriented exactly once. Structural break blocks are encoded
as break-only paragraphs; consecutive breaks and ordinary empty paragraphs
round-trip. Break-only paragraphs with spacing or typing styles retain their
formatted empty paragraphs. Supported documents reopen through the importer without warnings.
Line multiples that cannot be expressed exactly in Word's 1/240-line units
produce approximation warnings. Adjacent equal runs are normalized semantically.

## Diagnostics and source retention (task-4 / desktop integration)

`DocxError` differentiates archive/XML/I/O failures, invalid packages,
resource limits, encrypted ZIP packages and unsupported package kinds.
OLE compound files return an actionable unsupported-package error because they
may be encrypted Office files or legacy binary DOC files; the codec does not
pretend to identify/decrypt their payload. ZIP64, split/SFX packages, macro/template
main parts, non-UTF-8 XML and unsupported ZIP compression are rejected with
conversion guidance. Invalid values, missing required parts, cyclic style chains,
duplicate parts/relationships, unsafe paths and malformed XML fail explicitly.

`ImportReport.warnings` identifies unsupported structures, approximations,
missing styles and discarded parts with feature/code/part location. Unknown
content is never promised to survive export. Tables/images/embedded objects are
omitted; hyperlink/field cached text and supported inserted-revision text may be
shown with warnings. Unknown extensions/attributes, lists, section differences,
headers/footers, reviewing and external relationships warn. External targets are
never fetched, and no macros or embedded objects are executed. Diagnostics are
bounded/deduplicated, with a final warning if additional detail is suppressed.

**Any import warning requires a converted-copy Save As.** Use
`requires_converted_copy(&report)` (equivalent to `!report.warnings.is_empty()`).
Retain the source path and warnings separately from the editable core document.
The desktop save handler must reject the source path **and aliases of that file**
(including symlinks/hard links), even after loss is acknowledged, and write a
separate destination. Keep the original source unchanged. Display export warnings
as well. The codec operates on caller-owned streams and cannot enforce path
identity or atomic replacement; those UI/file tests belong to task-4. No shared
manifest, lockfile, core or interface changes are needed.

## Resource bounds

- Compressed archive: 64 MiB; expanded aggregate: 64 MiB; each part: 8 MiB.
- Central directory: 1,024 entries / 1 MiB, checked before ZIP table allocation;
  part names: 4,096 bytes. CRC verification occurs during bounded in-memory reads.
- Each parsed XML part: 200,000 events and 64 nesting levels; no DTD/entity
  declarations. Names/namespace URIs: 256 bytes; 128 attributes per element and
  128 active namespace bindings; attribute values: 64 KiB.
- Styles: 4,096 definitions, inheritance depth 64, cycles rejected.
- Diagnostics: at most 512 warnings. No archive path is extracted to disk.

Limits reject oversized input rather than partially importing it. Export validates
both the core model and generated XML limits before writing package bytes. I/O
failure may leave the supplied output stream partial: callers must use their
failure-safe temporary-save/replace workflow.

## References

Implementation references Microsoft's
[WordprocessingML structure documentation](https://learn.microsoft.com/en-us/office/open-xml/word/structure-of-a-wordprocessingml-document),
[paragraph styles documentation](https://learn.microsoft.com/en-us/office/open-xml/word/how-to-create-and-add-a-paragraph-style-to-a-word-processing-document),
[spacing attributes](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.spacingbetweenlines.line),
[toggle property behavior](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/f7130225-2368-48f3-acae-a9d278d0fb25), and
[MS-DOCX: Word Extensions to Office Open XML](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/b839fe1f-e1ca-4fa6-8c26-5954d0abbccd).
MS-DOCX extends the base format; unrepresented extension content is diagnosed,
not retained. This implementation does not claim full schema conformance or
pixel-identical Word rendering.

## Focused verification

All fixtures under `tests/fixtures` are self-authored XML; tests assemble real ZIP
packages in memory. They cover supported semantic round trips, inherited styles,
Unicode, alternate/strict namespace URIs, relationship-based part locations,
malformed/missing parts, encrypted ZIP flags, unknown content, resource limits,
and exported package declarations. Native Word interoperability is not tested.

Run `cargo test -p folio-docx --locked` and
`cargo clippy -p folio-docx --all-targets --no-deps --locked -- -D warnings`.
For this checkout Rust 1.90.0, rustfmt/clippy, caches, targets and temporary files
are confined to ignored `.tools/`. Set `CARGO_HOME=.tools/cargo`,
`RUSTUP_HOME=.tools/rustup`, `CARGO_TARGET_DIR=.tools/target` and `TMPDIR=.tools/tmp`
to absolute crate-local paths and prepend `.tools/cargo/bin` to the process PATH.
