use crate::Account;
use graphene_core::{AccountId, Result};
use std::{collections::BTreeMap, sync::Mutex};

pub trait AccountRepository: Send + Sync {
    fn list(&self) -> Result<Vec<Account>>;
    fn get(&self, account_id: AccountId) -> Result<Option<Account>>;
    fn put(&self, account: &Account) -> Result<()>;
    fn delete(&self, account_id: AccountId) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct InMemoryAccountRepository {
    accounts: Mutex<BTreeMap<String, Account>>,
}

impl InMemoryAccountRepository {
    #[must_use]
    pub fn new_for_tests() -> Self {
        Self::default()
    }
}

impl AccountRepository for InMemoryAccountRepository {
    fn list(&self) -> Result<Vec<Account>> {
        Ok(self
            .accounts
            .lock()
            .expect("account fixture store poisoned")
            .values()
            .cloned()
            .collect())
    }

    fn get(&self, account_id: AccountId) -> Result<Option<Account>> {
        Ok(self
            .accounts
            .lock()
            .expect("account fixture store poisoned")
            .get(&account_id.to_string())
            .cloned())
    }

    fn put(&self, account: &Account) -> Result<()> {
        account.validate()?;
        self.accounts
            .lock()
            .expect("account fixture store poisoned")
            .insert(account.id.to_string(), account.clone());
        Ok(())
    }

    fn delete(&self, account_id: AccountId) -> Result<()> {
        self.accounts
            .lock()
            .expect("account fixture store poisoned")
            .remove(&account_id.to_string());
        Ok(())
    }
}
