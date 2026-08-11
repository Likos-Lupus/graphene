use crate::{DataRoot, atomic::write_new_atomic};
use graphene_core::{AccountId, ErrorCode, ErrorKind, GrapheneError, Result};
use graphene_platform::{ManagedRelativePath, ensure_managed_directory};
use serde::{Serialize, de::DeserializeOwned};
use std::{fs, io::ErrorKind as IoErrorKind, path::PathBuf};

const MAX_ACCOUNT_RECORD_BYTES: u64 = 32 * 1024;
pub const ACCOUNT_RECORD_SCHEMA_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct AccountEnvelope<T> {
    schema_version: u32,
    account: T,
}

#[derive(Debug, Clone)]
pub struct AccountDocumentStore {
    root: PathBuf,
}

impl AccountDocumentStore {
    pub fn new(data_root: &DataRoot) -> Result<Self> {
        let root = ensure_managed_directory(
            data_root.path(),
            &ManagedRelativePath::new("config/accounts")?,
        )?;
        Ok(Self { root })
    }

    #[must_use]
    pub fn record_path(&self, account_id: AccountId) -> PathBuf {
        self.root.join(format!("{account_id}.json"))
    }

    pub fn list_ids(&self) -> Result<Vec<AccountId>> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(storage_read)? {
            let entry = entry.map_err(storage_read)?;
            let metadata = entry.file_type().map_err(storage_read)?;
            if metadata.is_symlink() || !metadata.is_file() {
                continue;
            }

            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }

            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };

            let id = stem.parse().map_err(|source| {
                GrapheneError::new(
                    ErrorCode::AuthPersistenceFailed,
                    ErrorKind::Storage,
                    "account record filename is not a valid AccountId",
                )
                .with_source(source)
            })?;
            ids.push(id);
        }

        ids.sort_by_key(ToString::to_string);
        Ok(ids)
    }

    pub fn read(&self, account_id: AccountId) -> Result<Option<Vec<u8>>> {
        let path = self.record_path(account_id);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == IoErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(storage_read(source)),
        };

        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAX_ACCOUNT_RECORD_BYTES
        {
            return Err(GrapheneError::new(
                ErrorCode::AuthPersistenceFailed,
                ErrorKind::Storage,
                "account record is unsafe or exceeds its size bound",
            ));
        }

        fs::read(path).map(Some).map_err(storage_read)
    }

    pub fn write(&self, account_id: AccountId, bytes: &[u8]) -> Result<()> {
        if bytes.len() as u64 > MAX_ACCOUNT_RECORD_BYTES {
            return Err(GrapheneError::new(
                ErrorCode::AuthPersistenceFailed,
                ErrorKind::Storage,
                "account record exceeds its size bound",
            ));
        }

        write_new_atomic(&self.record_path(account_id), bytes).map_err(|error| {
            GrapheneError::new(
                ErrorCode::AuthPersistenceFailed,
                ErrorKind::Storage,
                "failed to atomically persist account record",
            )
            .with_source(error)
        })
    }

    pub fn read_json<T: DeserializeOwned>(&self, account_id: AccountId) -> Result<Option<T>> {
        let Some(bytes) = self.read(account_id)? else {
            return Ok(None);
        };

        let envelope: AccountEnvelope<T> = serde_json::from_slice(&bytes).map_err(|source| {
            GrapheneError::new(
                ErrorCode::AuthPersistenceFailed,
                ErrorKind::Storage,
                "account record JSON is malformed",
            )
            .with_source(source)
        })?;
        if envelope.schema_version != ACCOUNT_RECORD_SCHEMA_VERSION {
            return Err(GrapheneError::new(
                ErrorCode::AuthPersistenceFailed,
                ErrorKind::Storage,
                "account record schema version is unsupported",
            )
            .with_context("schema_version", envelope.schema_version.to_string()));
        }

        Ok(Some(envelope.account))
    }

    pub fn write_json<T: Serialize>(&self, account_id: AccountId, account: &T) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&AccountEnvelope {
            schema_version: ACCOUNT_RECORD_SCHEMA_VERSION,
            account,
        })
        .map_err(|source| {
            GrapheneError::new(
                ErrorCode::AuthPersistenceFailed,
                ErrorKind::Storage,
                "failed to serialize account record",
            )
            .with_source(source)
        })?;
        self.write(account_id, &bytes)
    }

    pub fn delete(&self, account_id: AccountId) -> Result<()> {
        match fs::remove_file(self.record_path(account_id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == IoErrorKind::NotFound => Ok(()),
            Err(source) => Err(GrapheneError::new(
                ErrorCode::AuthPersistenceFailed,
                ErrorKind::Storage,
                "failed to remove account record",
            )
            .with_source(source)),
        }
    }
}

fn storage_read(source: std::io::Error) -> GrapheneError {
    GrapheneError::new(
        ErrorCode::AuthPersistenceFailed,
        ErrorKind::Storage,
        "failed to read account repository",
    )
    .with_source(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct FixtureAccount {
        label: String,
    }

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("graphene-account-store-{label}-{unique}"))
    }

    #[test]
    fn versioned_account_document_round_trips_and_replaces_atomically() {
        let path = temp_root("roundtrip");
        let root = DataRoot::initialize(&path).expect("data root");
        let store = AccountDocumentStore::new(&root).expect("account store");
        let id = AccountId::new();
        let first = FixtureAccount {
            label: "first".into(),
        };
        store.write_json(id, &first).expect("first write");
        assert_eq!(
            store.read_json::<FixtureAccount>(id).expect("read"),
            Some(first)
        );

        let second = FixtureAccount {
            label: "second".into(),
        };
        store.write_json(id, &second).expect("replacement write");
        assert_eq!(
            store
                .read_json::<FixtureAccount>(id)
                .expect("read replacement"),
            Some(second)
        );
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn future_account_schema_is_rejected_safely() {
        let path = temp_root("future-schema");
        let root = DataRoot::initialize(&path).expect("data root");
        let store = AccountDocumentStore::new(&root).expect("account store");
        let id = AccountId::new();
        fs::write(
            store.record_path(id),
            br#"{"schema_version":999,"account":{"label":"future"}}"#,
        )
        .expect("fixture write");
        let error = store
            .read_json::<FixtureAccount>(id)
            .expect_err("future schema must fail");
        assert_eq!(error.code, ErrorCode::AuthPersistenceFailed);
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn malformed_account_json_is_rejected_safely() {
        let path = temp_root("malformed");
        let root = DataRoot::initialize(&path).expect("data root");
        let store = AccountDocumentStore::new(&root).expect("account store");
        let id = AccountId::new();
        fs::write(store.record_path(id), b"{not-json").expect("fixture write");
        let error = store
            .read_json::<FixtureAccount>(id)
            .expect_err("malformed record must fail");
        assert_eq!(error.code, ErrorCode::AuthPersistenceFailed);
        let _ = fs::remove_dir_all(path);
    }
}
