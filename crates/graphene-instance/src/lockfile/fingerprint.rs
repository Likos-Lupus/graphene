use crate::{EffectiveInstanceConfig, InstallReceipt, InstanceLockfile};
use graphene_core::{InstanceId, Sha256Digest};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Cryptographic state fingerprint computed over all executable and repairable instance metadata.
///
/// Ensures optimistic concurrency and stale-plan protection across launch and repair operations.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceStateFingerprint(Sha256Digest);

impl InstanceStateFingerprint {
    /// Computes the deterministic state fingerprint.
    #[must_use]
    pub fn compute(
        instance_id: InstanceId,
        receipt: &InstallReceipt,
        lockfile: Option<&InstanceLockfile>,
        config: Option<&EffectiveInstanceConfig>,
    ) -> Self {
        let mut hasher = Sha256::new();

        // 1. Instance identity
        hasher.update(instance_id.to_string().as_bytes());

        // 2. Receipt metadata
        hasher.update(receipt.schema_version.to_be_bytes());
        hasher.update(receipt.requested_version.as_bytes());
        hasher.update(receipt.main_class.as_bytes());
        hasher.update(receipt.java_requirement.major_version.to_be_bytes());
        if let Some(hint) = &receipt.java_requirement.component_hint {
            hasher.update(hint.as_bytes());
        }
        for component in &receipt.components {
            hasher.update(component.uid.as_bytes());
            hasher.update(component.version.as_bytes());
            hasher.update(component.provider.as_bytes());
        }
        hasher.update(receipt.client.path.as_str().as_bytes());
        if let Some(sha1) = receipt.client.integrity.sha1() {
            hasher.update(sha1.as_bytes());
        }
        if let Some(sha256) = receipt.client.integrity.sha256() {
            hasher.update(sha256.as_bytes());
        }
        for lib in &receipt.libraries {
            hasher.update(lib.coordinate.as_bytes());
            if let Some(cp) = &lib.classpath {
                hasher.update(cp.path.as_str().as_bytes());
            }
        }
        hasher.update(receipt.natives_directory.as_str().as_bytes());
        hasher.update(receipt.assets_root.as_str().as_bytes());
        hasher.update(receipt.asset_index.path.as_str().as_bytes());

        // 3. Lockfile metadata (when present)
        if let Some(lock) = lockfile {
            hasher.update(lock.schema_version.to_be_bytes());
            for art in &lock.artifacts {
                hasher.update(art.logical_key.as_bytes());
                hasher.update(art.destination.as_str().as_bytes());
                if let Some(sha1) = art.integrity.sha1() {
                    hasher.update(sha1.as_bytes());
                }
                if let Some(sha256) = art.integrity.sha256() {
                    hasher.update(sha256.as_bytes());
                }
                if let Some(size) = art.expected_size {
                    hasher.update(size.to_be_bytes());
                }
            }
            for generated in &lock.generated_outputs {
                hasher.update(generated.destination.as_str().as_bytes());
                hasher.update(generated.sha256.as_bytes());
                hasher.update(generated.size.to_be_bytes());
            }
            for entry in &lock.content {
                hasher.update(entry.entry_id.as_bytes());
                hasher.update(entry.kind.as_bytes());
                if let Some(p) = &entry.provider {
                    hasher.update(p.as_bytes());
                }
                if let Some(pid) = &entry.project_id {
                    hasher.update(pid.as_bytes());
                }
                if let Some(vid) = &entry.version_id {
                    hasher.update(vid.as_bytes());
                }
                if let Some(fid) = &entry.file_id {
                    hasher.update(fid.as_bytes());
                }
                hasher.update(entry.artifact_logical_key.as_bytes());
                hasher.update(entry.destination.as_str().as_bytes());
                hasher.update(if entry.enabled { [1u8] } else { [0u8] });
                for dep in &entry.dependencies {
                    hasher.update(dep.target.as_bytes());
                    hasher.update(dep.relation.as_bytes());
                }
            }
        }

        // 4. Launch-affecting configuration (when present)
        if let Some(cfg) = config {
            if let Some(java) = &cfg.java_override {
                hasher.update(java.as_bytes());
            }
            if let Some(res) = &cfg.resolution {
                hasher.update(res.width.to_be_bytes());
                hasher.update(res.height.to_be_bytes());
            }
            if let Some(mem) = &cfg.memory {
                if let Some(min) = mem.min_memory_mib {
                    hasher.update(min.to_be_bytes());
                }
                if let Some(max) = mem.max_memory_mib {
                    hasher.update(max.to_be_bytes());
                }
            }
            for arg in &cfg.jvm_args {
                hasher.update(arg.as_bytes());
            }
            for arg in &cfg.game_args {
                hasher.update(arg.as_bytes());
            }
            for (k, v) in &cfg.env {
                hasher.update(k.as_bytes());
                hasher.update(v.as_bytes());
            }
        }

        let hash_bytes: [u8; 32] = hasher.finalize().into();
        Self(Sha256Digest::from_bytes(hash_bytes))
    }

    /// Returns the underlying SHA-256 digest.
    #[must_use]
    pub fn digest(&self) -> &Sha256Digest {
        &self.0
    }
}

impl fmt::Display for InstanceStateFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for InstanceStateFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("InstanceStateFingerprint")
            .field(&self.0.to_string())
            .finish()
    }
}
