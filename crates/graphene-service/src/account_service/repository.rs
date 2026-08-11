use graphene_auth::{Account, AccountRepository};
use graphene_core::{AccountId, ErrorCode, ErrorKind, GrapheneError, Result};
use graphene_storage::{AccountDocumentStore, DataRoot};

pub(crate) struct StorageAccountRepository {
    documents: AccountDocumentStore,
}

impl StorageAccountRepository {
    pub(crate) fn new(root: &DataRoot) -> Result<Self> {
        Ok(Self {
            documents: AccountDocumentStore::new(root)?,
        })
    }

    fn validate_loaded(&self, expected: AccountId, account: Account) -> Result<Account> {
        account.validate()?;
        if account.id != expected {
            return Err(GrapheneError::new(
                ErrorCode::AuthPersistenceFailed,
                ErrorKind::Storage,
                "account record identity does not match its repository path",
            ));
        }
        Ok(account)
    }
}

impl AccountRepository for StorageAccountRepository {
    fn list(&self) -> Result<Vec<Account>> {
        let mut accounts = Vec::new();
        for id in self.documents.list_ids()? {
            let account: Account = self.documents.read_json(id)?.ok_or_else(|| {
                GrapheneError::new(
                    ErrorCode::AuthPersistenceFailed,
                    ErrorKind::Storage,
                    "account disappeared while listing repository",
                )
            })?;
            accounts.push(self.validate_loaded(id, account)?);
        }
        accounts.sort_by_key(|account| account.id.to_string());
        Ok(accounts)
    }

    fn get(&self, account_id: AccountId) -> Result<Option<Account>> {
        let Some(account) = self.documents.read_json::<Account>(account_id)? else {
            return Ok(None);
        };

        Ok(Some(self.validate_loaded(account_id, account)?))
    }

    fn put(&self, account: &Account) -> Result<()> {
        account.validate()?;
        self.documents.write_json(account.id, account)
    }

    fn delete(&self, account_id: AccountId) -> Result<()> {
        self.documents.delete(account_id)
    }
}
