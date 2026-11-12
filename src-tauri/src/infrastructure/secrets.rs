use keyring::{Entry, Error as KeyringError};
use thiserror::Error;

const SERVICE: &str = "com.focusos.app";
const NIM_API_KEY: &str = "nvidia-nim-api-key";

#[derive(Debug, Error)]
pub enum SecretError {
    #[error("API key cannot be empty")]
    Empty,
    #[error("Operating system credential store error: {0}")]
    Keyring(String),
}

fn entry() -> Result<Entry, SecretError> {
    Entry::new(SERVICE, NIM_API_KEY).map_err(|error| SecretError::Keyring(error.to_string()))
}

pub fn store_nim_api_key(api_key: &str) -> Result<(), SecretError> {
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(SecretError::Empty);
    }
    entry()?
        .set_password(api_key)
        .map_err(|error| SecretError::Keyring(error.to_string()))
}

pub fn load_nim_api_key() -> Result<String, SecretError> {
    entry()?
        .get_password()
        .map_err(|error| SecretError::Keyring(error.to_string()))
}

pub fn has_nim_api_key() -> Result<bool, SecretError> {
    match entry()?.get_password() {
        Ok(_) => Ok(true),
        Err(KeyringError::NoEntry) => Ok(false),
        Err(error) => Err(SecretError::Keyring(error.to_string())),
    }
}

pub fn remove_nim_api_key() -> Result<(), SecretError> {
    match entry()?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(SecretError::Keyring(error.to_string())),
    }
}
