# Roadmap

Project author: hozaifahejaz.

## Foundation — implemented

- MIT workspace, original branding, pinned manifests and lockfile.
- Document model, Unicode-safe selection/editing, formatting, and page settings.
- Shared commands, undo/redo, literal search/replace, and saved-content dirty state.
- Bounded DOCX subset conversion and a paginated rich-text desktop app.
- Focused core tests, contributor setup and sourced feature inventory.

## First milestone — in progress

- Integrated DOCX roundtrip tests and command-driven desktop editing are implemented.
- Complete native OS IME/accessibility and external Word/LibreOffice fidelity checks.
- Run the configured Windows/macOS/Linux CI and remaining native end-to-end checks
  against [milestone criteria](docs/MILESTONE-1.md).

## Later milestones — deferred

- Expand tables/images, named styles/templates, numbering, multiple sections,
  headers/footers, and references deliberately with fidelity tests.
- Evaluate reviewing, proofing, accessibility, print/PDF, and extensions.
- Evaluate collaboration/cloud and browser/mobile after native editing is stable.

The [inventory](docs/FEATURES.md) records current limits. No complete Word parity
or release dates are promised; cloud and AI functionality are not implemented.
