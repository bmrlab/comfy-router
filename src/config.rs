use std::{env, str::FromStr};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub env: String,
    pub username: String,
    pub password: String,
    pub workflow_history_limit: usize,
    pub workflow_pending_limit: usize,
    pub cache_dir: String,
    pub root_dir: String,
    pub record_path: String,
    pub max_cache_bytes: u64,
}

trait FromEnvWithDefault: Sized {
    fn from_env_or_default(key: &str, default: Self) -> Self;
}

impl FromEnvWithDefault for u16 {
    fn from_env_or_default(key: &str, default: Self) -> Self {
        env::var(key)
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(default)
    }
}

impl FromEnvWithDefault for usize {
    fn from_env_or_default(key: &str, default: Self) -> Self {
        env::var(key)
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(default)
    }
}

impl FromEnvWithDefault for u64 {
    fn from_env_or_default(key: &str, default: Self) -> Self {
        env::var(key)
            .ok()
            .and_then(|val| val.parse().ok())
            .unwrap_or(default)
    }
}

impl FromEnvWithDefault for String {
    fn from_env_or_default(key: &str, default: Self) -> Self {
        env::var(key).unwrap_or(default)
    }
}

impl<T> FromEnvWithDefault for Option<T>
where
    T: FromStr,
{
    fn from_env_or_default(key: &str, default: Self) -> Self {
        env::var(key)
            .ok()
            .and_then(|val| val.parse().ok())
            .or(default)
    }
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            host: String::from_env_or_default("COMFY_ROUTER__HOST", "0.0.0.0".into()),
            port: u16::from_env_or_default("COMFY_ROUTER__PORT", 8080),
            username: String::from_env_or_default("COMFY_ROUTER__USERNAME", "admin".into()),
            password: String::from_env_or_default("COMFY_ROUTER__PASSWORD", "admin".into()),
            workflow_history_limit: usize::from_env_or_default("COMFY_ROUTER__HISTORY_LIMIT", 50),
            workflow_pending_limit: usize::from_env_or_default("COMFY_ROUTER__PENDING_LIMIT", 25),
            env: String::from_env_or_default("COMFY_ROUTER__ENV", "dev".into()),
            cache_dir: String::from_env_or_default(
                "COMFY_ROUTER__DOWNLOAD__CACHE_DIR",
                "/tmp/cache".into(),
            ),
            root_dir: String::from_env_or_default(
                "COMFY_ROUTER__DOWNLOAD__ROOT_DIR",
                "/tmp/model".into(),
            ),
            record_path: String::from_env_or_default(
                "COMFY_ROUTER__DOWNLOAD__RECORD_PATH",
                "/tmp/record.json".into(),
            ),
            max_cache_bytes: u64::from_env_or_default(
                "COMFY_ROUTER__DOWNLOAD__MAX_CACHE_BYTES",
                1024 * 1024 * 1024 * 64,
            ),
        }
    }
}
