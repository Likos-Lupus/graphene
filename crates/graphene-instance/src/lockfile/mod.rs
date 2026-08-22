mod fingerprint;
mod model;
#[cfg(test)]
mod tests;
mod validate;

pub use fingerprint::InstanceStateFingerprint;
pub use model::{
    InstanceLockfile, LOCKFILE_SCHEMA_VERSION, LockedArtifact, LockedGeneratedOutput,
    LockedMaterializationScope, LockedNativeExtraction, MAX_LOCKED_ARTIFACTS,
    MAX_LOCKED_COMPONENTS, MAX_LOCKED_EXTRACTIONS, MAX_LOCKED_OUTPUTS,
};
