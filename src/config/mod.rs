pub mod args;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default = "default_signaling_url")]
    pub signaling_url: String,
    #[serde(default = "default_identity_path")]
    pub identity_path: String,
    #[serde(default = "default_devices_path")]
    pub devices_path: String,
    #[serde(default)]
    pub pin: Option<String>,
    #[serde(default = "default_ice_servers")]
    pub ice_servers: Vec<String>,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("signaling_url", &self.signaling_url)
            .field("identity_path", &self.identity_path)
            .field("devices_path", &self.devices_path)
            .field("pin", &self.pin.as_ref().map(|_| "[REDACTED]"))
            .field("ice_servers", &self.ice_servers)
            .finish()
    }
}

fn default_ice_servers() -> Vec<String> {
    vec!["stun:stun.l.google.com:19302".to_owned(), "stun:stun1.l.google.com:19302".to_owned()]
}

fn default_signaling_url() -> String {
    "wss://localhost:8443/ws".to_owned()
}

fn default_identity_path() -> String {
    "identity.json".to_owned()
}

fn default_devices_path() -> String {
    "devices.json".to_owned()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            signaling_url: default_signaling_url(),
            identity_path: default_identity_path(),
            devices_path: default_devices_path(),
            pin: None,
            ice_servers: default_ice_servers(),
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let defaults = Self::default();
        let settings = config::Config::builder()
            .set_default("signaling_url", defaults.signaling_url)?
            .set_default("identity_path", defaults.identity_path)?
            .set_default("devices_path", defaults.devices_path)?
            .set_default("pin", defaults.pin)?
            .set_default("ice_servers", defaults.ice_servers)?
            .add_source(config::File::with_name("blnk.toml").required(false))
            .add_source(config::Environment::with_prefix("BLNK").separator("_"))
            .build()?;

        Ok(settings.try_deserialize()?)
    }
}

/// Persistent metadata for a known peer. Secrets and private keys are never stored here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceRecord {
    pub id: String,
    pub endpoint: String,
    pub last_seen_unix: u64,
}

/// Small metadata-only registry used by the CLI MVP.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceRegistry {
    pub devices: Vec<DeviceRecord>,
}

impl DeviceRegistry {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents = fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&contents)?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension(format!("tmp-{}", uuid::Uuid::new_v4().simple()));
        let payload = serde_json::to_vec_pretty(self)?;
        fs::write(&temporary, payload)?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        Ok(())
    }

    pub fn upsert(&mut self, id: impl Into<String>, endpoint: impl Into<String>) {
        let id = id.into();
        let endpoint = endpoint.into();
        let last_seen_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();
        if let Some(device) = self.devices.iter_mut().find(|device| device.id == id) {
            device.endpoint = endpoint;
            device.last_seen_unix = last_seen_unix;
        } else {
            self.devices
                .push(DeviceRecord { id, endpoint, last_seen_unix });
        }
        self.devices.sort_by(|left, right| left.id.cmp(&right.id));
    }

    pub fn find(&self, id: &str) -> Option<&DeviceRecord> {
        self.devices.iter().find(|device| device.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_load_uses_defaults_when_file_is_missing() {
        let cfg = Config::load().expect("default config should load");

        assert_eq!(cfg.signaling_url, "wss://localhost:8443/ws");
        assert_eq!(cfg.identity_path, "identity.json");
        assert_eq!(cfg.devices_path, "devices.json");
        assert_eq!(cfg.pin, None);
    }

    #[test]
    fn config_debug_redacts_pin() {
        let cfg = Config {
            pin: Some("secret123".to_owned()),
            ..Config::default()
        };
        let debug_str = format!("{cfg:?}");
        assert!(!debug_str.contains("secret123"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn config_supports_explicit_values() {
        let cfg = Config {
            signaling_url: "wss://example.test/ws".to_owned(),
            identity_path: "/tmp/blnk-identity.json".to_owned(),
            devices_path: "/tmp/blnk-devices.json".to_owned(),
            pin: Some("1234".to_owned()),
            ice_servers: vec!["stun:custom.stun.org:3478".to_owned()],
        };

        assert_eq!(cfg.signaling_url, "wss://example.test/ws");
        assert_eq!(cfg.identity_path, "/tmp/blnk-identity.json");
        assert_eq!(cfg.devices_path, "/tmp/blnk-devices.json");
        assert_eq!(cfg.pin.as_deref(), Some("1234"));
        assert_eq!(cfg.ice_servers, vec!["stun:custom.stun.org:3478"]);
    }

    #[test]
    fn registry_upserts_and_sorts_without_secrets() {
        let mut registry = DeviceRegistry::default();
        registry.upsert("zeta", "local://zeta");
        registry.upsert("alpha", "local://alpha");
        registry.upsert("zeta", "local://updated");

        assert_eq!(registry.devices.len(), 2);
        assert_eq!(registry.devices[0].id, "alpha");
        assert_eq!(
            registry.find("zeta").map(|device| device.endpoint.as_str()),
            Some("local://updated")
        );
    }

    #[test]
    fn registry_round_trips_metadata() {
        let path = std::env::temp_dir().join(format!("blnk-devices-{}.json", uuid::Uuid::new_v4()));
        let mut registry = DeviceRegistry::default();
        registry.upsert("fixture", "local://fixture");
        registry.save(&path).expect("registry should save");
        let loaded = DeviceRegistry::load(&path).expect("registry should load");
        let _ = fs::remove_file(path);

        assert_eq!(loaded.devices.len(), 1);
        assert_eq!(loaded.devices[0].id, "fixture");
    }
}
