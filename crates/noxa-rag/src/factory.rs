use std::sync::Arc;

use crate::config::{EmbedProviderConfig, RagConfig, VectorStoreConfig};
use crate::embed::{DynEmbedProvider, TeiProvider};
use crate::error::RagError;
use crate::store::{DynVectorStore, QdrantStore, VectorStore};

/// Build the embed provider from config, running a startup probe.
///
/// Returns `(provider, dims)` so callers can use the probed dimensions directly
/// without a redundant second probe.
///
/// Fails fast at startup if the provider is unavailable or returns wrong dimensions.
/// `is_available()` and `dimensions()` are concrete methods on the provider struct,
/// called here directly (not via dyn dispatch).
pub async fn build_embed_provider(
    config: &RagConfig,
) -> Result<(DynEmbedProvider, usize), RagError> {
    match &config.embed_provider {
        EmbedProviderConfig::Tei {
            url,
            model,
            auth_token,
            dimensions: config_dims,
            ..
        } => {
            let client = reqwest::Client::new();
            let provider =
                TeiProvider::new_with_probe(url.clone(), model.clone(), client, auth_token.clone())
                    .await
                    .map_err(|e| RagError::Config(format!("TEI startup probe failed: {e}")))?;

            if !provider.is_available().await {
                return Err(RagError::Config(format!(
                    "TEI provider at {} is not available (GET /health failed). \
                     Ensure TEI is running with --pooling last-token for Qwen3-0.6B.",
                    url
                )));
            }

            let probed_dims = provider.dimensions();
            if probed_dims == 0 {
                return Err(RagError::Config(
                    "TEI provider returned 0 dimensions — probe failed silently".to_string(),
                ));
            }

            // MRL dimension override validation.
            if let Some(target) = config_dims {
                if *target == 0 {
                    return Err(RagError::Config(
                        "configured dimensions must be > 0".to_string(),
                    ));
                }
                if *target > probed_dims {
                    return Err(RagError::Config(format!(
                        "configured dimensions {target} exceeds model output {probed_dims} — \
                         set dimensions ≤ {probed_dims} or remove the override"
                    )));
                }
            }

            let effective_dims = config_dims.unwrap_or(probed_dims);
            let provider = provider.with_configured_dimensions(*config_dims);

            // Non-fatal: warn if TEI max_batch_tokens < BATCH_SIZE × target_tokens.
            provider
                .check_max_batch_tokens(config.chunker.target_tokens)
                .await;

            tracing::info!(
                provider = provider.name(),
                probed_dims,
                effective_dims,
                url = %url,
                "embed provider ready"
            );

            Ok((Arc::new(provider), effective_dims))
        }
        EmbedProviderConfig::OpenAi { .. } => Err(RagError::Config(
            "OpenAI embed provider reached factory unexpectedly after config validation"
                .to_string(),
        )),
        EmbedProviderConfig::VoyageAi { .. } => Err(RagError::Config(
            "VoyageAI embed provider reached factory unexpectedly after config validation"
                .to_string(),
        )),
    }
}

/// Build the vector store from config, running collection lifecycle checks.
///
/// Creates the collection if missing; fails if existing collection has wrong dimensions.
/// `collection_exists()` and `create_collection()` are concrete methods on QdrantStore,
/// called here directly (not via dyn dispatch).
pub async fn build_vector_store(
    config: &RagConfig,
    embed_dims: usize,
) -> Result<DynVectorStore, RagError> {
    match &config.vector_store {
        VectorStoreConfig::Qdrant {
            url,
            collection,
            api_key,
        } => {
            // Resolve api_key: config value takes precedence, env var as fallback.
            let resolved_api_key = api_key
                .clone()
                .or_else(|| std::env::var("NOXA_RAG_QDRANT_API_KEY").ok());

            let store = QdrantStore::new(
                url,
                collection.clone(),
                resolved_api_key,
                config.uuid_namespace,
            )?;

            // Collection lifecycle: create if missing, validate dims if exists.
            if store.collection_exists().await? {
                // Validate that the existing collection's vector size matches embed dims.
                // Fail fast if there is a mismatch rather than letting upsert fail later
                // with a confusing Qdrant error.
                let existing_dims = store.collection_vector_size().await?;
                if existing_dims != embed_dims {
                    return Err(RagError::Config(format!(
                        "existing Qdrant collection {collection:?} has {existing_dims}-dim vectors \
                         but embed provider outputs {embed_dims} dims — delete the collection or \
                         switch to a matching embed model"
                    )));
                }
                tracing::info!(
                    collection = %collection,
                    dims = existing_dims,
                    "collection already exists with matching dimensions"
                );
                store.reconcile_landed_file_metadata_indexes().await?;
                tracing::info!(
                    collection = %collection,
                    "reconciled landed file metadata indexes"
                );
            } else {
                tracing::info!(collection = %collection, dims = embed_dims, "creating collection");
                store.create_collection(embed_dims).await?;
            }

            tracing::info!(
                store = store.name(),
                collection = %collection,
                url = %url,
                "vector store ready"
            );

            Ok(Arc::new(store))
        }
        VectorStoreConfig::InMemory => Err(RagError::Config(
            "InMemory vector store reached factory unexpectedly after config validation"
                .to_string(),
        )),
    }
}
