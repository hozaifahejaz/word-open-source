//! Folio's UI-independent rich text model and transactional editing commands.
//! See `docs/INTERFACES.md` in the workspace for the integration contract.
mod case;
mod editor;
mod model;

pub use case::TextCase;
pub use editor::*;
pub use model::*;
