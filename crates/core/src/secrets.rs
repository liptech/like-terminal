use std::collections::HashMap;
use std::sync::Mutex;

use uuid::Uuid;

pub const SERVICE: &str = "like-terminal";

pub fn account_password(session: Uuid) -> String {
    format!("session-{session}-password")
}

pub fn account_passphrase(session: Uuid) -> String {
    format!("session-{session}-passphrase")
}

pub fn account_totp() -> String {
    "totp".to_string()
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("keyring indisponível: {0}")]
    Keyring(String),
    #[error("segredo não encontrado")]
    NotFound,
}

pub trait SecretStore {
    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError>;
    fn get(&self, account: &str) -> Result<Option<String>, SecretError>;
    fn delete(&self, account: &str) -> Result<(), SecretError>;

    fn get_required(&self, account: &str) -> Result<String, SecretError> {
        self.get(account)?.ok_or(SecretError::NotFound)
    }
}

pub struct KeyringStore {
    service: String,
}

impl KeyringStore {
    pub fn new() -> Self {
        Self {
            service: SERVICE.to_string(),
        }
    }
}

impl Default for KeyringStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeyringStore {
    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
        let entry = keyring::Entry::new(&self.service, account)
            .map_err(|error| SecretError::Keyring(error.to_string()))?;
        entry
            .set_password(secret)
            .map_err(|error| SecretError::Keyring(error.to_string()))
    }

    fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        let entry = keyring::Entry::new(&self.service, account)
            .map_err(|error| SecretError::Keyring(error.to_string()))?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(SecretError::Keyring(error.to_string())),
        }
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        let entry = keyring::Entry::new(&self.service, account)
            .map_err(|error| SecretError::Keyring(error.to_string()))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(SecretError::Keyring(error.to_string())),
        }
    }
}

#[derive(Default)]
pub struct MemoryStore {
    values: Mutex<HashMap<String, String>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemoryStore {
    fn set(&self, account: &str, secret: &str) -> Result<(), SecretError> {
        self.values
            .lock()
            .expect("lock poisoned")
            .insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn get(&self, account: &str) -> Result<Option<String>, SecretError> {
        Ok(self
            .values
            .lock()
            .expect("lock poisoned")
            .get(account)
            .cloned())
    }

    fn delete(&self, account: &str) -> Result<(), SecretError> {
        self.values.lock().expect("lock poisoned").remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip() {
        let store = MemoryStore::new();
        let account = account_password(Uuid::new_v4());
        assert!(store.get(&account).unwrap().is_none());
        store.set(&account, "segredo").unwrap();
        assert_eq!(store.get(&account).unwrap().as_deref(), Some("segredo"));
        store.delete(&account).unwrap();
        assert!(store.get(&account).unwrap().is_none());
    }

    #[test]
    fn account_names_are_distinct() {
        let id = Uuid::new_v4();
        assert_ne!(account_password(id), account_passphrase(id));
        assert!(account_password(id).contains(&id.to_string()));
    }

    #[test]
    fn get_required_fails_when_missing() {
        let store = MemoryStore::new();
        assert!(matches!(
            store.get_required("nope"),
            Err(SecretError::NotFound)
        ));
    }
}
