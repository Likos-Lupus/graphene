use super::LoaderProvider;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use graphene_minecraft::LoaderKind;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Default)]
pub struct LoaderProviderRegistry {
    providers: BTreeMap<LoaderKind, Arc<dyn LoaderProvider>>,
}

impl std::fmt::Debug for LoaderProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoaderProviderRegistry")
            .field("provider_count", &self.providers.len())
            .finish()
    }
}

impl LoaderProviderRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, provider: Arc<dyn LoaderProvider>) -> Result<()> {
        let kind = provider.kind();
        if self.providers.insert(kind, provider).is_some() {
            return Err(GrapheneError::new(
                ErrorCode::LoaderProviderUnavailable,
                ErrorKind::Minecraft,
                "loader provider is registered more than once",
            )
            .with_context("loader", kind.to_string()));
        }

        Ok(())
    }

    pub fn provider(&self, kind: LoaderKind) -> Result<Arc<dyn LoaderProvider>> {
        self.providers.get(&kind).cloned().ok_or_else(|| {
            GrapheneError::new(
                ErrorCode::LoaderProviderUnavailable,
                ErrorKind::Minecraft,
                "loader provider is unavailable",
            )
            .with_context("loader", kind.to_string())
        })
    }
}
