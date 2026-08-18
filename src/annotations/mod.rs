mod reanchor;
mod schema;
mod store;

pub use reanchor::{ReanchorOutcome, reanchor_annotation};
pub use schema::{
    Anchor, AnchorState, Annotation, AnnotationStatus, Sidecar, SidecarDocument, SourcePosition,
    SourceRange, capture_anchor,
};
pub use store::{SIDECAR_SCHEMA_VERSION, SidecarStore, SidecarStoreError, default_sidecar_path};
