use crate::instance_service::repository::InstanceRepository;
use graphene_core::{ErrorCode, ErrorKind, GrapheneError, InstanceId, Result};
use graphene_instance::{InstanceInventoryEntry, InstanceStatus};
use graphene_storage::is_instance_infrastructure_name;
use std::{fs, str::FromStr};

/// Scans the `instances/` directory and returns all discoverable instance records.
///
/// Ensures:
/// - Only valid instance-ID directory names are processed.
/// - Deterministic sorting by `InstanceId`.
/// - Infrastructure entries (`.locks`, `.staging`, `.trash`, `.install-locks`) are ignored.
/// - Symbolic links are never followed.
/// - One malformed instance does not abort the entire inventory scan.
pub fn scan_inventory(repository: &InstanceRepository) -> Result<Vec<InstanceInventoryEntry>> {
    let instances_dir = repository.paths().instances_dir();
    if !instances_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(&instances_dir).map_err(|source| {
        GrapheneError::new(
            ErrorCode::FileOpenFailed,
            ErrorKind::Filesystem,
            "failed to read instances directory",
        )
        .with_context("path", instances_dir.display().to_string())
        .with_source(source)
    })?;

    let mut candidate_ids: Vec<InstanceId> = Vec::new();

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let file_name = entry.file_name();
        let name_str = match file_name.to_str() {
            Some(s) => s,
            None => continue,
        };

        if is_instance_infrastructure_name(name_str) {
            continue;
        }

        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }

        if let Ok(id) = InstanceId::from_str(name_str) {
            candidate_ids.push(id);
        }
    }

    // Deterministic sorted order
    candidate_ids.sort();

    let mut inventory = Vec::with_capacity(candidate_ids.len());

    for id in candidate_ids {
        let entry = match repository.load_committed(id) {
            Ok(committed) => {
                let display_name = committed.descriptor.display_name;
                let version = committed.receipt.requested_version;
                let components = committed.receipt.components;
                match committed.status {
                    InstanceStatus::Ready => {
                        InstanceInventoryEntry::ready(id, display_name, version, components)
                    }
                    InstanceStatus::Legacy => {
                        InstanceInventoryEntry::legacy(id, display_name, version, components)
                    }
                    InstanceStatus::Invalid => {
                        InstanceInventoryEntry::invalid(id, "invalid metadata status")
                    }
                }
            }
            Err(err) => InstanceInventoryEntry::invalid(id, err.message().to_string()),
        };
        inventory.push(entry);
    }

    Ok(inventory)
}
