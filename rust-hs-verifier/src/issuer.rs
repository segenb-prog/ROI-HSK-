use ed25519_dalek::{Keypair, PublicKey, SecretKey};
use serde::{Deserialize, Serialize};
use std::path::Path;
use zeroize::{Zeroize, ZeroizeOnDrop};
use chrono::{DateTime, Utc};
use rand::rngs::OsRng;

#[derive(Debug, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct KeyEntry {
    #[zeroize(skip)]
    pub key_id: String,
    #[zeroize(skip)]
    pub public_key: String,
    pub secret_key: String,
    #[zeroize(skip)]
    pub activated_at: DateTime<Utc>,
    #[zeroize(skip)]
    pub expires_at: Option<DateTime<Utc>>,
    #[zeroize(skip)]
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Keyring {
    #[zeroize(skip)]
    pub version: String,
    #[zeroize(skip)]
    pub created_at: DateTime<Utc>,
    pub keys: Vec<KeyEntry>,
    #[zeroize(skip)]
    pub active_key_index: usize,
}

impl Keyring {
    pub fn current_key_id(&self) -> String {
        self.keys.get(self.active_key_index)
            .map(|k| k.key_id.clone())
            .unwrap_or_default()
    }
    
    pub fn active_public_key(&self) -> PublicKeyBytes {
        self.keys.get(self.active_key_index)
            .and_then(|k| {
                base64::decode(&k.public_key).ok()
                    .and_then(|b| b.try_into().ok())
            })
            .unwrap_or([0u8; 32])
    }
    
    pub fn get_active_keypair(&self) -> Result<Keypair, Box<dyn std::error::Error>> {
        let entry = self.keys.get(self.active_key_index)
            .ok_or("No active key")?;
        
        let secret_bytes = base64::decode(&entry.secret_key)?;
        let secret = SecretKey::from_bytes(&secret_bytes)?;
        
        let public_bytes = base64::decode(&entry.public_key)?;
        let public = PublicKey::from_bytes(&public_bytes)?;
        
        Ok(Keypair { secret, public })
    }
    
    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(path, perms)?;
        }
        
        Ok(())
    }
    
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let data = std::fs::read_to_string(path)?;
        let keyring: Keyring = serde_json::from_str(&data)?;
        Ok(keyring)
    }
    
    pub fn rotate_key(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        
        let new_entry = KeyEntry {
            key_id: format!("key-{}", Uuid::new_v4().to_string()[..8].to_string()),
            public_key: base64::encode(keypair.public.to_bytes()),
            secret_key: base64::encode(keypair.secret.to_bytes()),
            activated_at: Utc::now(),
            expires_at: Some(Utc::now() + chrono::Duration::days(365)),
            revoked_at: None,
        };
        
        if let Some(active) = self.keys.get_mut(self.active_key_index) {
            active.revoked_at = Some(Utc::now());
        }
        
        self.keys.push(new_entry);
        self.active_key_index = self.keys.len() - 1;
        
        Ok(())
    }
}

pub type PublicKeyBytes = [u8; 32];

pub fn generate_keyring() -> Result<Keyring, Box<dyn std::error::Error>> {
    let mut csprng = OsRng;
    let keypair = Keypair::generate(&mut csprng);
    
    let entry = KeyEntry {
        key_id: format!("key-{}", Uuid::new_v4().to_string()[..8].to_string()),
        public_key: base64::encode(keypair.public.to_bytes()),
        secret_key: base64::encode(keypair.secret.to_bytes()),
        activated_at: Utc::now(),
        expires_at: Some(Utc::now() + chrono::Duration::days(365)),
        revoked_at: None,
    };
    
    Ok(Keyring {
        version: "1.0".to_string(),
        created_at: Utc::now(),
        keys: vec![entry],
        active_key_index: 0,
    })
}

pub fn load_keyring() -> Result<Keyring, Box<dyn std::error::Error>> {
    let keyring_path = std::env::var("HSK_KEYRING_PATH")
        .unwrap_or_else(|_| "keyring.json".to_string());
    
    Keyring::load(Path::new(&keyring_path))
}

pub fn get_signing_key() -> Result<Keypair, Box<dyn std::error::Error>> {
    let keyring = load_keyring()?;
    keyring.get_active_keypair()
}

use uuid::Uuid;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generate_keyring() {
        let keyring = generate_keyring().unwrap();
        
        assert_eq!(keyring.keys.len(), 1);
        assert_eq!(keyring.active_key_index, 0);
        assert!(!keyring.current_key_id().is_empty());
    }

    #[test]
    fn test_keyring_save_and_load() {
        let keyring = generate_keyring().unwrap();
        let temp_file = NamedTempFile::new().unwrap();
        
        keyring.save(temp_file.path()).unwrap();
        let loaded = Keyring::load(temp_file.path()).unwrap();
        
        assert_eq!(loaded.current_key_id(), keyring.current_key_id());
    }

    #[test]
    fn test_key_rotation() {
        let mut keyring = generate_keyring().unwrap();
        let original_key_id = keyring.current_key_id();
        
        keyring.rotate_key().unwrap();
        
        assert_eq!(keyring.keys.len(), 2);
        assert_ne!(keyring.current_key_id(), original_key_id);
        assert!(keyring.keys[0].revoked_at.is_some());
    }
}
