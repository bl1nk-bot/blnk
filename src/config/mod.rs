pub mod args;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Config {
    #[serde(default = "default_signaling_url")]
    pub signaling_url: String,
    #[serde(default = "default_identity_path")]
    pub identity_path: String,
    #[serde(default)]
    pub pin: Option<String>,
}

fn default_signaling_url() -> String {
    "wss://localhost:8443/ws".to_owned()
}

fn default_identity_path() -> String {
    "identity.json".to_owned()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            signaling_url: default_signaling_url(),
            identity_path: default_identity_path(),
            pin: None,
        }
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        Self::load_with_sources(true, true)
    }

    #[cfg(test)]
    fn load_without_external_sources() -> anyhow::Result<Self> {
        Self::load_with_sources(false, false)
    }

    #[cfg(test)]
    fn load_with_environment_only() -> anyhow::Result<Self> {
        Self::load_with_sources(false, true)
    }

    fn load_with_sources(include_file: bool, include_environment: bool) -> anyhow::Result<Self> {
        let defaults = Self::default();
        let mut builder = config::Config::builder()
            .set_default("signaling_url", defaults.signaling_url)?
            .set_default("identity_path", defaults.identity_path)?
            .set_default("pin", defaults.pin)?;

        if include_file {
            builder = builder.add_source(config::File::with_name("blnk.toml").required(false));
        }
        if include_environment {
            builder = builder.add_source(config::Environment::with_prefix("BLNK"));
        }

        let settings = builder.build()?;
        Ok(settings.try_deserialize()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_load_uses_defaults_without_external_sources() {
        let cfg = Config::load_without_external_sources().expect("default config should load");

        assert_eq!(cfg.signaling_url, "wss://localhost:8443/ws");
        assert_eq!(cfg.identity_path, "identity.json");
        assert_eq!(cfg.pin, None);
    }

    #[test]
    fn config_environment_overrides_flat_fields() {
        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _lock = ENV_LOCK
            .lock()
            .expect("environment lock should not be poisoned");
        let keys = ["BLNK_SIGNALING_URL", "BLNK_IDENTITY_PATH", "BLNK_PIN"];
        let previous = keys.map(|key| (key, std::env::var(key).ok()));

        // The test serializes access through ENV_LOCK before mutating process-wide variables.
        unsafe {
            std::env::set_var("BLNK_SIGNALING_URL", "wss://env.example/ws");
            std::env::set_var("BLNK_IDENTITY_PATH", "/tmp/env-identity.json");
            std::env::set_var("BLNK_PIN", "9876");
        }
        let cfg = Config::load_with_environment_only().expect("environment config should load");

        for (key, value) in previous {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }

        assert_eq!(cfg.signaling_url, "wss://env.example/ws");
        assert_eq!(cfg.identity_path, "/tmp/env-identity.json");
        assert_eq!(cfg.pin.as_deref(), Some("9876"));
    }

    #[test]
    fn config_supports_explicit_values() {
        let cfg = Config {
            signaling_url: "wss://example.test/ws".to_owned(),
            identity_path: "/tmp/blnk-identity.json".to_owned(),
            pin: Some("1234".to_owned()),
        };

        assert_eq!(cfg.signaling_url, "wss://example.test/ws");
        assert_eq!(cfg.identity_path, "/tmp/blnk-identity.json");
        assert_eq!(cfg.pin.as_deref(), Some("1234"));
    }
}
