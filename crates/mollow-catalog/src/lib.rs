mod catalog;
mod normalize;

pub use catalog::{CatalogError, EnrichmentInput, enrich, render_diagram};
pub use normalize::{matches_pattern, normalize_model};
