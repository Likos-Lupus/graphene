use graphene_content::{ContentProvider, ContentProviderId};
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, Result};
use std::{collections::BTreeMap, sync::Arc};

/// Registry of available remote content providers (e.g. Modrinth, CurseForge).
#[derive(Clone, Default)]
pub struct ContentProviderRegistry {
    providers: BTreeMap<ContentProviderId, Arc<dyn ContentProvider>>,
}

impl std::fmt::Debug for ContentProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentProviderRegistry")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ContentProviderRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a content provider. Rejects duplicate provider IDs.
    pub fn register(&mut self, provider: Arc<dyn ContentProvider>) -> Result<()> {
        let id = provider.id().clone();
        if self.providers.insert(id.clone(), provider).is_some() {
            return Err(GrapheneError::new(
                ErrorCode::ContentProviderUnavailable,
                ErrorKind::Content,
                format!("content provider {id} is registered more than once"),
            ));
        }
        Ok(())
    }

    /// Looks up a provider by its unique identifier.
    pub fn get(&self, id: &ContentProviderId) -> Result<Arc<dyn ContentProvider>> {
        self.providers.get(id).cloned().ok_or_else(|| {
            GrapheneError::new(
                ErrorCode::ContentProviderUnavailable,
                ErrorKind::Content,
                format!("content provider {id} is not registered or unavailable"),
            )
        })
    }

    /// Iterates over all registered providers in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = (&ContentProviderId, &Arc<dyn ContentProvider>)> {
        self.providers.iter()
    }

    /// Returns whether any provider is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::modrinth::{ModrinthContentProvider, ModrinthProviderConfig};
    use graphene_content::ContentSearchQuery;
    use graphene_network::NetworkClient;

    fn test_network() -> NetworkClient {
        NetworkClient::new(graphene_network::NetworkConfig::default()).expect("client")
    }

    fn modrinth_provider() -> Arc<dyn ContentProvider> {
        Arc::new(
            ModrinthContentProvider::new(test_network(), ModrinthProviderConfig::production())
                .expect("provider"),
        )
    }

    #[test]
    fn duplicate_provider_ids_are_rejected() {
        let mut registry = ContentProviderRegistry::new();
        registry.register(modrinth_provider()).expect("first");
        assert!(registry.register(modrinth_provider()).is_err());
    }

    #[test]
    fn lookup_is_deterministic_and_absent_providers_fail_typed() {
        let mut registry = ContentProviderRegistry::new();
        registry.register(modrinth_provider()).expect("register");

        let id = ContentProviderId::new(ContentProviderId::MODRINTH).expect("id");
        let provider = registry.get(&id).expect("found");
        assert_eq!(provider.id(), &id);

        // Deterministic iteration order.
        let ids: Vec<String> = registry
            .iter()
            .map(|(k, _)| k.as_str().to_string())
            .collect();
        assert_eq!(ids, vec!["modrinth".to_string()]);

        // Absent providers fail with a typed error.
        let missing = ContentProviderId::new("nonexistent").expect("id");
        let error = registry.get(&missing).expect_err("absent");
        assert_eq!(error.code, ErrorCode::ContentProviderUnavailable);

        // Capability-driven flow works without branching on concrete adapter types.
        assert!(provider.capabilities().search);
        assert!(!provider.capabilities().requires_credentials);
        let _ = ContentSearchQuery::default();
    }
}
