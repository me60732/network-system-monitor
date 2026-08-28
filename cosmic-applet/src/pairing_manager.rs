use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;

use hex::serde as hex_serde;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};

/// Manages the set of paired machines and their derived keys.
///
/// This struct handles the storage and retrieval of pairing information,
/// including machine IDs, shared keys (derived via ECDH), and timestamps.
pub struct PairingManager {
    /// Mapping from machine ID to its pairing information.
    paired_machines: HashMap<String, PairingInfo>,
    /// Path to the TOML configuration file where pairings are stored.
    config_path: PathBuf,
    /// Receiver's Ed25519 signing key (also usable as X25519 secret for ECDH).
    receiver_key: SigningKey,
}

/// Information about a paired machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingInfo {
    /// Unique identifier of the machine.
    pub machine_id: String,
    /// The 32-byte ChaCha20 key derived via ECDH exchange (hex-encoded in TOML).
    #[serde(with = "hex_serde")]
    pub shared_key: [u8; 32],
    /// Timestamp when the pairing was established.
    pub paired_at: DateTime<Utc>,
    /// Hostname or IP address of the machine.
    pub host: String,
}

/// Request to pair a new machine, received via UDP.
#[derive(Debug, Clone)]
pub struct PairingRequest {
    /// Unique identifier of the requesting machine.
    pub machine_id: String,
    /// X25519 public key from the remote machine (32 bytes).
    pub sender_pubkey: [u8; 32],
    /// IP address string of the sender.
    pub host: String,
    /// Timestamp when the request was received (for 60-second timeout).
    pub received_at: std::time::Instant,
}

/// TOML storage format for paired machines.
#[derive(Serialize, Deserialize)]
struct PairingStorage {
    #[serde(rename = "paired_machines")]
    paired_machines: Vec<PairingInfo>,
}

impl PairingManager {
    /// Creates a new PairingManager that will load/save pairings from the given path.
    ///
    /// Loads existing pairings from disk if the file exists. Also loads or generates
    /// the receiver's Ed25519 keypair for ECDH key derivation.
    ///
    /// # Arguments
    /// * `config_path` - Path to the TOML file for persistent storage of pairings.
    pub fn new(config_path: PathBuf) -> Self {
        let paired_machines = Self::load_from_disk(&config_path).unwrap_or_default();

        // Load or generate receiver keypair at ~/.config/cosmic-applet/receiver.key
        let receiver_key_path = config_path.parent().unwrap().join("receiver.key");
        let receiver_key_bytes = Self::load_receiver_key(&receiver_key_path);
        let receiver_key = SigningKey::from_bytes(&receiver_key_bytes);

        PairingManager {
            paired_machines,
            config_path,
            receiver_key,
        }
    }

    /// Checks if a machine with the given ID is already paired.
    ///
    /// # Arguments
    /// * `machine_id` - The unique identifier of the machine to check.
    ///
    /// # Returns
    /// `true` if the machine is paired, `false` otherwise.
    pub fn is_paired(&self, machine_id: &str) -> bool {
        self.paired_machines.contains_key(machine_id)
    }

    /// Retrieves the shared key for a paired machine, if it exists.
    ///
    /// # Arguments
    /// * `machine_id` - The unique identifier of the machine.
    ///
    /// # Returns
    /// Some reference to the 32-byte ChaCha20 key if the machine is paired, None otherwise.
    pub fn get_key(&self, machine_id: &str) -> Option<&[u8; 32]> {
        self.paired_machines
            .get(machine_id)
            .map(|info| &info.shared_key)
    }

    /// Adds a new pairing or updates an existing one.
    ///
    /// Derives the shared key using ECDH between the receiver's Ed25519 secret
    /// and the sender's X25519 public key.
    ///
    /// # Arguments
    /// * `machine_id` - The unique identifier of the machine.
    /// * `sender_pubkey_bytes` - The sender's X25519 public key (32 bytes).
    /// * `host` - The hostname or IP address of the machine.
    ///
    /// # Returns
    /// Ok(()) on success, or an error if key derivation fails.
    pub fn add_pairing(
        &mut self,
        machine_id: String,
        sender_pubkey_bytes: &[u8; 32],
        host: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Derive shared secret using ECDH
        let receiver_secret = X25519Secret::from(self.receiver_key.to_bytes());
        let sender_pubkey = X25519PublicKey::from(*sender_pubkey_bytes);
        let shared_secret = receiver_secret.diffie_hellman(&sender_pubkey);
        let chacha_key: [u8; 32] = *shared_secret.as_bytes();

        let info = PairingInfo {
            machine_id: machine_id.clone(),
            shared_key: chacha_key,
            paired_at: Utc::now(),
            host,
        };
        self.paired_machines.insert(machine_id, info);

        // Persist to disk
        Self::save_to_disk(&self.config_path, &self.paired_machines)?;

        Ok(())
    }

    /// Removes a pairing by machine ID.
    ///
    /// # Arguments
    /// * `machine_id` - The unique identifier of the machine to remove.
    ///
    /// # Returns
    /// Ok(()) on success, or an error if saving fails.
    pub fn remove_pairing(&mut self, machine_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.paired_machines.remove(machine_id);

        // Persist to disk
        Self::save_to_disk(&self.config_path, &self.paired_machines)?;

        Ok(())
    }

    /// Returns the count of pending (unpaired) machines waiting for pairing requests.
    ///
    /// For now, this returns 0 as we don't track pending separately from paired.
    pub fn pending_count(&self) -> usize {
        // TODO: Implement if pending state tracking is added
        0
    }

    /// Loads existing pairings from the TOML file on disk.
    fn load_from_disk(
        path: &PathBuf,
    ) -> Result<HashMap<String, PairingInfo>, Box<dyn std::error::Error>> {
        if !path.exists() {
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(path)?;
        let storage: PairingStorage = toml::from_str(&content)?;

        Ok(storage
            .paired_machines
            .into_iter()
            .map(|info| (info.machine_id.clone(), info))
            .collect())
    }

    /// Saves current pairings to the TOML file on disk.
    fn save_to_disk(
        path: &PathBuf,
        pairs: &HashMap<String, PairingInfo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let storage = PairingStorage {
            paired_machines: pairs.values().cloned().collect(),
        };

        let toml_str = toml::to_string(&storage)?;
        fs::write(path, toml_str)?;

        // Set file permissions to 0600 (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(path, perms)?;
        }

        Ok(())
    }

    /// Loads the receiver's Ed25519 keypair from disk.
    fn load_receiver_key(path: &PathBuf) -> [u8; 32] {
        if path.exists() {
            match fs::read(path) {
                Ok(bytes) if bytes.len() == 32 => {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    return key;
                }
                Ok(_) => log::warn!("receiver.key has invalid length, regenerating"),
                Err(e) => log::warn!("Failed to read receiver.key: {}, regenerating", e),
            }
        }
        // Generate new key if loading fails
        let new_key = SigningKey::generate(&mut rand::rngs::OsRng);
        Self::save_receiver_key(&new_key, path).expect("Failed to save receiver key");
        new_key.to_bytes()
    }

    /// Saves the receiver's Ed25519 keypair to disk with 0600 permissions.
    fn save_receiver_key(key: &SigningKey, path: &PathBuf) -> std::io::Result<()> {
        let bytes = key.to_bytes();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, bytes)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(path, perms)?;
        }

        Ok(())
    }

    /// Returns the receiver's X25519 public key as a hex-encoded string.
    /// This is used for ECDH encryption with remote senders.
    pub fn get_receiver_x25519_pubkey(&self) -> String {
        let x25519_secret = X25519Secret::from(self.receiver_key.to_bytes());
        let x25519_pub = X25519PublicKey::from(&x25519_secret);
        hex::encode(x25519_pub.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use getrandom::getrandom;
    use std::path::PathBuf;

    #[test]
    fn test_pairing_storage() {
        // Create temp directory
        let config_path = PathBuf::from("/tmp/test-pairing.toml");

        // Generate sender keypair for test using getrandom to create secret bytes
        let mut sender_secret_bytes = [0u8; 32];
        getrandom(&mut sender_secret_bytes).expect("Failed to generate random bytes");
        let sender_pubkey = X25519PublicKey::from(sender_secret_bytes);

        // Create manager and add pairing
        let mut manager = PairingManager::new(config_path.clone());
        manager
            .add_pairing(
                "test-machine".to_string(),
                &sender_pubkey.to_bytes(),
                "127.0.0.1".to_string(),
            )
            .expect("Failed to add pairing");

        // Verify in-memory state
        assert!(manager.is_paired("test-machine"));
        let key = manager.get_key("test-machine").expect("Key should exist");
        assert_eq!(key.len(), 32);

        // Reload from disk and verify
        let reload_manager = PairingManager::new(config_path);
        assert!(reload_manager.is_paired("test-machine"));
        let reloaded_key = reload_manager
            .get_key("test-machine")
            .expect("Key should exist after reload");
        assert_eq!(reloaded_key, key);
    }

    #[test]
    fn test_ecdh_key_derivation() {
        // Generate two X25519 secrets from random bytes
        let mut alice_secret_bytes = [0u8; 32];
        let mut bob_secret_bytes = [0u8; 32];
        getrandom(&mut alice_secret_bytes).expect("Failed to generate random bytes");
        getrandom(&mut bob_secret_bytes).expect("Failed to generate random bytes");

        let alice_secret = X25519Secret::from(alice_secret_bytes);
        let bob_secret = X25519Secret::from(bob_secret_bytes);

        // Derive public keys from the secrets (not from raw bytes directly)
        let alice_pubkey = X25519PublicKey::from(&alice_secret);
        let bob_pubkey = X25519PublicKey::from(&bob_secret);

        // Each side computes the shared secret using their secret + the other's public key
        let alice_shared = alice_secret.diffie_hellman(&bob_pubkey);
        let bob_shared = bob_secret.diffie_hellman(&alice_pubkey);

        // Both sides must arrive at the same shared secret
        assert_eq!(alice_shared.as_bytes(), bob_shared.as_bytes());
    }

    #[test]
    fn test_unknown_sender_not_paired() {
        let config_path = PathBuf::from("/tmp/test-unknown-pairing.toml");

        let manager = PairingManager::new(config_path);

        assert!(!manager.is_paired("unknown-machine"));
        assert!(manager.get_key("unknown-machine").is_none());
    }

    #[test]
    fn test_remove_pairing() {
        // Generate sender keypair
        let mut sender_secret_bytes = [0u8; 32];
        getrandom(&mut sender_secret_bytes).expect("Failed to generate random bytes");
        let sender_pubkey = X25519PublicKey::from(sender_secret_bytes);

        let config_path = PathBuf::from("/tmp/test-remove-pairing.toml");

        let mut manager = PairingManager::new(config_path.clone());

        manager
            .add_pairing(
                "test-machine".to_string(),
                &sender_pubkey.to_bytes(),
                "127.0.0.1".to_string(),
            )
            .expect("Failed to add pairing");

        assert!(manager.is_paired("test-machine"));

        // Remove pairing
        manager
            .remove_pairing("test-machine")
            .expect("Failed to remove pairing");

        assert!(!manager.is_paired("test-machine"));
        assert!(manager.get_key("test-machine").is_none());

        // Verify removal persisted
        let reload_manager = PairingManager::new(config_path);
        assert!(!reload_manager.is_paired("test-machine"));
    }
}
