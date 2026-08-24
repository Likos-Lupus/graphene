use graphene_core::{ErrorCode, ErrorKind, GrapheneError, InstanceId, Result, Sha256Digest};
use graphene_instance::ManagedRelativePath;
use graphene_platform::replace_file_safely;
use graphene_storage::{read_document_bounded, write_document_atomic};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const JOURNAL_SCHEMA_VERSION: u32 = 1;
pub const MAX_JOURNAL_BYTES: usize = 512 * 1024; // 512 KiB

/// Transaction state recorded in the durable content journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContentTransactionPhase {
    /// Journal written and all new files staged; filesystem mutation about to begin.
    Prepared,
    /// Files published and new lockfile committed; cleanup pending.
    Committed,
}

/// A staged-to-final mapping in the transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalStagedMapping {
    pub staged_path: String,
    pub destination: ManagedRelativePath,
}

/// A quarantined file mapping for rollback capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalQuarantineMapping {
    pub original_path: ManagedRelativePath,
    pub quarantine_path: String,
}

/// A rename mapping for enable/disable operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalRenameMapping {
    pub from: ManagedRelativePath,
    pub to: ManagedRelativePath,
}

/// Durable content mutation journal stored at `instances/<id>/.graphene/content-journal.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentMutationJournal {
    pub schema_version: u32,
    pub operation_id: String,
    pub instance_id: InstanceId,
    pub phase: ContentTransactionPhase,
    pub old_lockfile_sha256: Option<Sha256Digest>,
    pub new_lockfile_sha256: Sha256Digest,
    pub staged_mappings: Vec<JournalStagedMapping>,
    pub quarantine_mappings: Vec<JournalQuarantineMapping>,
    pub rename_mappings: Vec<JournalRenameMapping>,
}

impl ContentMutationJournal {
    pub fn journal_path(instance_root: &Path) -> PathBuf {
        instance_root.join(".graphene").join("content-journal.json")
    }

    pub fn load(instance_root: &Path) -> Result<Option<Self>> {
        let path = Self::journal_path(instance_root);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = read_document_bounded(&path, MAX_JOURNAL_BYTES)?;
        let journal: Self = serde_json::from_slice(&bytes).map_err(|source| {
            GrapheneError::new(
                ErrorCode::ContentRecoveryFailed,
                ErrorKind::Storage,
                "failed to parse content mutation journal",
            )
            .with_source(source)
        })?;
        Ok(Some(journal))
    }

    pub fn write_durable(&self, instance_root: &Path) -> Result<()> {
        let path = Self::journal_path(instance_root);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| {
            GrapheneError::new(
                ErrorCode::ContentMutationFailed,
                ErrorKind::Storage,
                "failed to serialize content mutation journal",
            )
            .with_source(source)
        })?;
        write_document_atomic(&path, &bytes)
    }

    pub fn delete_durable(&self, instance_root: &Path) -> Result<()> {
        let path = Self::journal_path(instance_root);
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }
}

use sha2::{Digest, Sha256};

fn compute_sha256(bytes: &[u8]) -> Sha256Digest {
    let hash: [u8; 32] = Sha256::digest(bytes).into();
    Sha256Digest::from_bytes(hash)
}

/// Recovers any pending content mutation journal found under an instance.
/// Must be executed under the `InstanceExclusiveLease`.
pub fn recover_content_journal(
    instance_root: &Path,
    current_lockfile_bytes: Option<&[u8]>,
) -> Result<()> {
    let journal = match ContentMutationJournal::load(instance_root)? {
        Some(j) => j,
        None => return Ok(()),
    };

    let current_sha256 = current_lockfile_bytes.map(compute_sha256);

    match journal.phase {
        ContentTransactionPhase::Prepared => {
            // Check if lockfile was already committed to the new state
            if current_sha256.as_ref() == Some(&journal.new_lockfile_sha256) {
                // Committed! Finish forward cleanup
                cleanup_transaction_staging(instance_root, &journal)?;
                journal.delete_durable(instance_root)?;
            } else {
                // Rollback toward old state:
                // 1. Rollback renames (in reverse order)
                for rm in journal.rename_mappings.iter().rev() {
                    let from_path = instance_root.join(rm.to.as_str());
                    let to_path = instance_root.join(rm.from.as_str());
                    if from_path.exists() && !to_path.exists() {
                        let _ = replace_file_safely(&from_path, &to_path);
                    }
                }

                // 2. Restore quarantined files back to their original destinations
                for qm in &journal.quarantine_mappings {
                    let q_path = instance_root.join(&qm.quarantine_path);
                    let orig_path = instance_root.join(qm.original_path.as_str());
                    if q_path.exists() {
                        if let Some(parent) = orig_path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        let _ = replace_file_safely(&q_path, &orig_path);
                    }
                }

                // 3. Remove staged publications that might have been published
                for sm in &journal.staged_mappings {
                    let dest = instance_root.join(sm.destination.as_str());
                    if dest.exists() {
                        let _ = fs::remove_file(dest);
                    }
                }

                // 4. Clean staging directory
                cleanup_transaction_staging(instance_root, &journal)?;
                journal.delete_durable(instance_root)?;
            }
        }
        ContentTransactionPhase::Committed => {
            // Already committed to desired state; finish cleanup
            cleanup_transaction_staging(instance_root, &journal)?;
            journal.delete_durable(instance_root)?;
        }
    }

    Ok(())
}

fn cleanup_transaction_staging(
    instance_root: &Path,
    _journal: &ContentMutationJournal,
) -> Result<()> {
    let staging_dir = instance_root
        .join(".graphene")
        .join("staging")
        .join("content");
    if staging_dir.exists() {
        let _ = fs::remove_dir_all(staging_dir);
    }
    let quarantine_dir = instance_root
        .join(".graphene")
        .join("quarantine")
        .join("content");
    if quarantine_dir.exists() {
        let _ = fs::remove_dir_all(quarantine_dir);
    }
    Ok(())
}

/// Optional test hook for injecting simulated failures during transaction execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentFaultPoint {
    BeforeJournalDurable,
    AfterJournalPrepared,
    AfterQuarantine,
    AfterPartialPublication,
    BeforeLockfileCommit,
    AfterLockfileCommitBeforeFinalize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphene_core::Sha256Digest;

    #[test]
    fn journal_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let journal = ContentMutationJournal {
            schema_version: JOURNAL_SCHEMA_VERSION,
            operation_id: "op-123".to_string(),
            instance_id: InstanceId::new(),
            phase: ContentTransactionPhase::Prepared,
            old_lockfile_sha256: None,
            new_lockfile_sha256: Sha256Digest::from_bytes([1u8; 32]),
            staged_mappings: vec![JournalStagedMapping {
                staged_path: ".graphene/staging/content/mod.jar".to_string(),
                destination: ManagedRelativePath::new(".minecraft/mods/mod.jar").unwrap(),
            }],
            quarantine_mappings: vec![],
            rename_mappings: vec![],
        };

        journal.write_durable(temp.path()).expect("write journal");
        let loaded = ContentMutationJournal::load(temp.path())
            .expect("load journal")
            .expect("some journal");
        assert_eq!(loaded, journal);

        journal.delete_durable(temp.path()).expect("delete");
        assert!(ContentMutationJournal::load(temp.path()).unwrap().is_none());
    }
}
