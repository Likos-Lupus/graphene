use graphene_core::{OperationController, Result};
use graphene_minecraft::{
    LoaderKind, LoaderProviderCapabilities, LoaderVersion, LoaderVersionSummary,
    MinecraftVersionId, ResolvedLoader,
};
use std::{future::Future, path::PathBuf, pin::Pin};

pub type LoaderProviderFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

pub trait LoaderProvider: Send + Sync {
    fn kind(&self) -> LoaderKind;

    fn capabilities(&self) -> LoaderProviderCapabilities;

    fn list_versions<'a>(
        &'a self,
        minecraft: &'a MinecraftVersionId,
        operation: &'a OperationController,
    ) -> LoaderProviderFuture<'a, Vec<LoaderVersionSummary>>;

    fn resolve_exact<'a>(
        &'a self,
        minecraft: &'a MinecraftVersionId,
        version: &'a LoaderVersion,
        operation: &'a OperationController,
    ) -> LoaderProviderFuture<'a, ResolvedLoader>;

    fn normalize_verified_installer<'a>(
        &'a self,
        resolved: ResolvedLoader,
        verified_installer: PathBuf,
        operation: &'a OperationController,
    ) -> LoaderProviderFuture<'a, ResolvedLoader> {
        let _ = verified_installer;
        let _ = operation;
        Box::pin(async move { Ok(resolved) })
    }
}
