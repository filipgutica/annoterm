mod reanchor;
mod schema;
mod store;

pub use reanchor::{ReanchorOutcome, reanchor_annotation, reanchor_annotation_with_snapshots};
pub use schema::{
    Anchor, AnchorState, Annotation, AnnotationStatus, NavigationHint, Sidecar, SidecarDocument,
    SourcePosition, SourceRange, capture_anchor,
};
pub use store::{SIDECAR_SCHEMA_VERSION, SidecarStore, SidecarStoreError, default_sidecar_path};
