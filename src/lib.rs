//! Re-exports for benchmarks.
//! THIS IS NOT A STABLE PUBLIC API. DO NOT RELY ON THIS.

mod visitors;

#[cfg(feature = "_bench")]
pub use visitors::render_thumbnail::RenderThumbnailVisitor;
