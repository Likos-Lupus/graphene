use graphene_core::{ErrorCode, ErrorKind, GrapheneError};

pub(crate) fn mc_error(code: ErrorCode, message: &'static str) -> GrapheneError {
    GrapheneError::new(code, ErrorKind::Minecraft, message)
}

pub(crate) fn library_error(message: &'static str) -> GrapheneError {
    mc_error(ErrorCode::MinecraftLibraryInvalid, message)
}
