use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

pub(super) fn provider_network_error(source: GrapheneError) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::MinecraftManifestInvalid,
        ErrorKind::Minecraft,
        "failed to acquire Minecraft provider metadata",
    )
    .with_context("network_code", source.code.as_str())
    .with_source(source)
}

pub(super) fn mc_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Minecraft, message)
}
