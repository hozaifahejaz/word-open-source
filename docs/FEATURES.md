# Feature inventory

This is a scoped Folio inventory, not exhaustive Word parity. **Supported** means
implemented within the stated scope; **Partial** means limited support; **Deferred**
means no implementation. The integrated 0.1.0 desktop and DOCX subset are implemented.
Implementation support is distinct from native-host verification; see
[acceptance evidence](ACCEPTANCE.md). This is a desktop foundation, not Word parity.

References reviewed **2026-10-09**. Word features vary by platform, subscription
and build; published help scopes are recorded below without invented build numbers.

## Reference baseline

- [Create a document](https://support.microsoft.com/en-us/word/training/create-a-document-in-word):
  Microsoft 365, Word 2024/2021; explicitly Windows desktop.
- [Design and edit](https://support.microsoft.com/en-us/word/training/design-and-edit-in-word):
  Microsoft 365 and Word 2024/2021/2019/2016 help scopes for formatting/proofing.
- [Mac quick start](https://download.microsoft.com/download/6/3/4/634e576c-136a-4638-b750-2f9d1b90a573/Word%20for%20MAC%20Quick%20Start%20Guide.pdf):
  Microsoft 365 for macOS; no exact build specified. Folio needs separate Mac tests.
- [Desktop/web comparison](https://support.microsoft.com/en-au/word/word-features-comparison-word-for-the-web-vs-desktop):
  current broad capability overview, not an OS-specific build matrix.
- [Web service description](https://learn.microsoft.com/en-us/office365/servicedescriptions/office-online-service-description/word-online):
  browser Microsoft 365 scope, distinct from desktop.
- [Mobile Copilot](https://support.microsoft.com/en-us/word/copilot/copilot-in-word-on-mobile-devices):
  iPad/iPhone/Android, subject to subscription and organization settings.
- [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/):
  Part 1 fifth edition December 2016 (markup), Part 2 December 2021 (packaging),
  Part 3 December 2015 (compatibility), Part 4 December 2016 (transitional features).
- [WordprocessingML structure](https://learn.microsoft.com/en-us/office/open-xml/word/structure-of-a-wordprocessingml-document):
  paragraphs/runs and related stories/parts; references ISO/IEC 29500:2016.
- [MS-DOCX](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/b839fe1f-e1ca-4fa6-8c26-5954d0abbccd):
  OOXML extensions, revision 23.0 dated 2026-08-18; not implicitly supported.
- [MS-DOC](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-doc/):
  Word 97–2003 binary format, distinct from ZIP/XML DOCX.

## Current state and later work

| Family | Status | Folio boundary | Reference |
| --- | --- | --- | --- |
| Paragraphs/styled text | Supported | Bold/italic/underline, family/size/RGB; native rich-text canvas with font fallback | Create document; ECMA Part 1 |
| Selection/editing | Supported | Grapheme-safe commands, selection, history, visual navigation; IME event support, OS verification limited | Design/edit; contracts |
| Editing toolbar/statistics | Supported | Labeled original icons, plain-text Cut/Copy/Paste, clear text formatting; document/selection whitespace-word and grapheme counts | Desktop guide |
| Find/replace | Supported | Literal case-sensitive paragraph-local, cross-run search; advanced modes deferred | Design/edit; contracts |
| Alignment/spacing | Supported | Four alignments; before/after and multiple/exact/at-least spacing; indent/tab stops deferred | Desktop/web; ECMA Part 1 |
| Page settings/breaks | Supported | One size/orientation/margin set and explicit breaks; automatic/explicit pagination implemented | Desktop/web; ECMA Part 1 |
| DOCX | Partial | Supported semantic subset import/export; inherited styles flattened; omissions/approximations warn | ECMA Parts 1–4; MS-DOCX |
| Tables/images | Deferred | No model/rendering; imports must warn | Create document; ECMA Part 1 |
| Styles/templates | Partial | Inherited named styles resolved on import and flattened; no named-style editor/templates | Web service; WordprocessingML |
| Lists | Deferred | No bullets, numbering definitions/hierarchy | Web service; ECMA Part 1 |
| Sections/columns | Partial | Single layout only; mixed sections/columns absent | Desktop/web; ECMA Part 1 |
| Headers/footers | Deferred | No repeating stories/page-number evaluation | WordprocessingML |
| References/fields | Deferred | No notes, TOC, citations, bibliography, captions or fields | Desktop/web; WordprocessingML |
| Reviewing | Deferred | No comments, tracking, acceptance/rejection or comparison | Mac quick start; Desktop/web |
| Collaboration | Deferred | Local editing only; no coauthoring/version history/sync | Create document; Web service |
| Proofing | Deferred | No spelling/grammar, dictionary, translation or thesaurus | Design/edit; Desktop/web |
| Mail merge | Deferred | No data sources, merge fields, envelopes/labels | Desktop/web; ECMA Part 1 |
| Printing/PDF | Deferred | No print pipeline or PDF export | Desktop/web |
| Accessibility | Partial | eframe backend enabled; custom editing semantics/screen-reader checks/audit pending | Desktop/web |
| Automation/add-ins | Partial | 14 MCP tools for live/background documents; no VBA/macros/Office add-ins execution | [MCP setup](MCP.md); Desktop/web |
| Legacy/other formats | Deferred | No .doc, .docm, .dot/.dotx, RTF or ODT codec | MS-DOC; ECMA |
| Cloud/AI | Deferred | No OneDrive/accounts, online services, dictation or AI assistant | Create document; Mobile Copilot |
| Browser/mobile | Deferred | Windows/macOS/Linux native first; web/iOS/Android front ends later | Web service; Mobile Copilot |

Folio does not bundle Word branding, templates or assets. Use original fixtures
and record supported elements, approximations and rejected features explicitly.
Unknown imported content must warn rather than quietly disappear on save.
Update statuses after implementation; planned work is not supported behavior.

## 0.1.0 verification boundary

The macOS 27.0 arm64 release bundle was exercised for text/Unicode paste,
selection/formatting, automatic and explicit pages, undo/redo, literal replacement,
DOCX save/reopen, unsaved decisions, failures and protected converted-copy saving.
All 56 workspace tests and serial formatting/clippy/release checks passed locally.
CI is configured for Windows/macOS/Linux; its results remain pending. Native
Windows/Linux behavior has not been tested from this Mac. Word/LibreOffice were
unavailable; textutil independently recovered exported text but did not validate
Word rendering. OS dead-key preedit/commit worked, while full CJK candidate selection
and cancellation, VoiceOver and accessibility auditing remain unverified.
Emoji composition, Urdu/bidi shaping and exact Word pagination remain fidelity
limitations. See [the detailed acceptance report](ACCEPTANCE.md).
