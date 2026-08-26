mod fingerprint;
mod model;
#[cfg(test)]
mod tests;
mod validate;

pub use fingerprint::InstanceStateFingerprint;
pub use model::{
    InstanceLockfile, LOCKFILE_SCHEMA_VERSION, LockedArtifact, LockedContentDependency,
    LockedContentEntry, LockedGeneratedOutput, LockedMaterializationScope, LockedNativeExtraction,
    LockedPackOrigin, MAX_LOCKED_ARTIFACTS, MAX_LOCKED_COMPONENTS, MAX_LOCKED_CONTENT,
    MAX_LOCKED_DEPENDENCIES_PER_ENTRY, MAX_LOCKED_EXTRACTIONS, MAX_LOCKED_OUTPUTS,
    MAX_PACK_ORIGIN_STRING_CHARS,
};
