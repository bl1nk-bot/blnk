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
        let defaults = Self::default();
        let settings = config::Config::builder()
            .set_default("signaling_url", defaults.signaling_url)?
            .set_default("identity_path", defaults.identity_path)?
            .set_default("pin", defaults.pin)?
            .add_source(config::File::with_name("blnk.toml").required(false))
            .add_source(config::Environment::with_prefix("BLNK").separator("_"))
            .build()?;

        Ok(settings.try_deserialize()?)
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
        assert_eq!(cfg.pin, None);
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
