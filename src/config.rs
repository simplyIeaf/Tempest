use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;
use crate::TempestError;

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub auth: AuthConfig,
    pub paths: PathConfig,
    pub wine: WineConfig,
    pub launcher: LauncherConfig,
    pub proton: ProtonConfig,
}

#[derive(Serialize, Deserialize)]
pub struct ProtonConfig {
    pub enabled: bool,
    pub umu: String,
    pub proton_path: String,
    pub game_id: String,
    pub store: String,
}

impl Default for ProtonConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            umu: "umu-run".to_string(),
            proton_path: "GE-Proton".to_string(),
            game_id: "umu-vortex".to_string(),
            store: "none".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct AuthConfig {
    pub session_token: Option<String>,
    pub username: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct PathConfig {
    pub wine_prefix: PathBuf,
    pub vortex_exe: PathBuf,
    pub log_file: PathBuf,
}

#[derive(Serialize, Deserialize)]
pub struct WineConfig {
    pub binary: String,
    pub env: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct LauncherConfig {
    pub filter_wine_noise: bool,
    pub auto_update: bool,
    pub use_esync: bool,
    pub use_fsync: bool,
    pub use_gamemode: bool,
    pub shader_cache: bool,
    pub fsr: u8,
}

impl Default for PathConfig {
    fn default() -> Self {
        let data = Config::data_dir();
        Self {
            wine_prefix: data.join("prefix"),
            vortex_exe: data.join("Vortex.exe"),
            log_file: Config::data_dir().join("tempest.log"),
        }
    }
}

impl Default for WineConfig {
    fn default() -> Self {
        Self {
            binary: "wine".to_string(),
            env: HashMap::new(),
        }
    }
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            filter_wine_noise: true,
            auto_update: true,
            use_esync: true,
            use_fsync: true,
            use_gamemode: false,
            shader_cache: true,
            fsr: 0,
        }
    }
}

impl Config {
    pub fn config_dir() -> PathBuf {
        match dirs::config_dir() {
            Some(dir) => dir.join("tempest"),
            None => home_dir().join(".config").join("tempest"),
        }
    }

    pub fn data_dir() -> PathBuf {
        match dirs::data_local_dir() {
            Some(dir) => dir.join("tempest"),
            None => home_dir().join(".local").join("share").join("tempest"),
        }
    }

    pub fn load() -> Self {
        let path = Self::config_dir().join("config.toml");
        if !path.exists() {
            return Self::default();
        }
        let contents = std::fs::read_to_string(&path).unwrap_or_default();

        #[derive(serde::Deserialize)]
        struct RawConfig {
            auth: RawAuth,
        }

        #[derive(serde::Deserialize)]
        struct RawAuth {
            session_token: Option<String>,
            #[allow(dead_code)]
            username: Option<String>,
        }

        let raw: RawConfig = toml::from_str(&contents).unwrap_or(RawConfig {
            auth: RawAuth {
                session_token: None,
                username: None,
            },
        });

        let mut cfg: Self = toml::from_str(&contents).unwrap_or_default();

        if let Some(ref token) = raw.auth.session_token
            && let Some(decrypted) = crate::crypto::decrypt(token)
        {
            cfg.auth.session_token = Some(decrypted);
        }

        cfg
    }

    pub fn save(&self) -> Result<(), TempestError> {
        let mut cloned = self.serialize_plain();

        if let Some(ref token) = cloned.auth.session_token.clone()
            && crate::crypto::decrypt(token).is_none()
        {
            cloned.auth.session_token = Some(crate::crypto::encrypt(token));
        }

        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let contents = toml::to_string_pretty(&cloned)
            .map_err(|e| TempestError::ConfigError(e.to_string()))?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    fn serialize_plain(&self) -> Self {
        Self {
            auth: AuthConfig {
                session_token: self.auth.session_token.clone(),
                username: self.auth.username.clone(),
            },
            paths: PathConfig {
                wine_prefix: self.paths.wine_prefix.clone(),
                vortex_exe: self.paths.vortex_exe.clone(),
                log_file: self.paths.log_file.clone(),
            },
            wine: WineConfig {
                binary: self.wine.binary.clone(),
                env: self.wine.env.clone(),
            },
            launcher: LauncherConfig {
                filter_wine_noise: self.launcher.filter_wine_noise,
                auto_update: self.launcher.auto_update,
                use_esync: self.launcher.use_esync,
                use_fsync: self.launcher.use_fsync,
                use_gamemode: self.launcher.use_gamemode,
                shader_cache: self.launcher.shader_cache,
                fsr: self.launcher.fsr,
            },
            proton: ProtonConfig {
                enabled: self.proton.enabled,
                umu: self.proton.umu.clone(),
                proton_path: self.proton.proton_path.clone(),
                game_id: self.proton.game_id.clone(),
                store: self.proton.store.clone(),
            },
        }
    }
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let cfg = Config::default();
        let serialized = toml::to_string_pretty(&cfg).unwrap();
        let loaded: Config = toml::from_str(&serialized).unwrap();
        assert_eq!(loaded.launcher.filter_wine_noise, cfg.launcher.filter_wine_noise);
        assert_eq!(loaded.launcher.auto_update, cfg.launcher.auto_update);
        assert_eq!(loaded.wine.binary, cfg.wine.binary);
    }
}
