use async_trait::async_trait;
use std::sync::Arc;

use crate::error::RagError;

/// Pluggable embedding provider.
///
/// Trait surface is minimal by design — only what ALL impls share.
/// `is_available()` and `dimensions()` are concrete methods on each provider struct,
/// called during factory startup probes (not via dyn dispatch).
#[async_trait]
pub trait EmbedProvider: Send + Sync {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, RagError>;
}

pub type DynEmbedProvider = Arc<dyn EmbedProvider + Send + Sync>;

pub mod tei;
pub use tei::TeiProvider;
