//! There is deliberately no file-based credential fallback.
pub struct Vault {
    entry: keyring::Entry,
}

impl Vault {
    pub fn new(client_id: &str) -> Result<Self, String> {
        crate::github::validate_client_id(client_id)?;
        Ok(Self {
            entry: keyring::Entry::new("com.pupontech.handy.dictionary.github", client_id)
                .map_err(|_| "credential_store")?,
        })
    }
    pub fn load(&self) -> Result<Option<String>, String> {
        match self.entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("credential_store".into()),
        }
    }
    pub fn save(&self, token: &str) -> Result<(), String> {
        self.entry
            .set_password(token)
            .map_err(|_| "credential_store".into())
    }
    pub fn delete(&self) -> Result<(), String> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("credential_store".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credential_roundtrip_and_disconnect() {
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        let vault = Vault::new("Iv1.fixture").unwrap();
        assert_eq!(vault.load().unwrap(), None);
        vault.save("fixture-token-not-a-real-credential").unwrap();
        assert_eq!(
            vault.load().unwrap().as_deref(),
            Some("fixture-token-not-a-real-credential")
        );
        vault.delete().unwrap();
        assert_eq!(vault.load().unwrap(), None);
    }
    #[test]
    #[ignore = "requires the real OS credential store; explicitly run on Windows CI"]
    fn native_credential_store_roundtrip() {
        let id = format!("Iv1.test{}", std::process::id());
        let vault = Vault::new(&id).unwrap();
        vault
            .save("synthetic-ci-marker-not-a-github-token")
            .unwrap();
        let readback = Vault::new(&id).unwrap().load().unwrap();
        vault.delete().unwrap();
        assert_eq!(
            readback.as_deref(),
            Some("synthetic-ci-marker-not-a-github-token")
        );
        assert_eq!(Vault::new(&id).unwrap().load().unwrap(), None);
    }
}
