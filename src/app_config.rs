//src/app_config.rs

use serde::Deserialize;
use std::env;
use config::{Config, ConfigError, File};
use std::convert::TryFrom;

#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    pub ipc_path: String,
    pub future_block_offset: u64,
    pub state_file: String,
    pub script_file: String,
    pub default_total_counted_seats: usize,
    pub default_max_earner_seats: usize,
    pub default_qualified_majority_threshold: f64,
    pub counted_vote_points: u32,
    pub uncounted_vote_points: u32,
    pub telegram: TelegramConfig,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TelegramConfig {
    pub chat_id: String,
    #[serde(skip)]
    pub token: String,
}

impl AppConfig {
    pub fn new() -> Result<Self, ConfigError> {
        let mut settings = Config::default();

        // Start off with default values
        settings.set_default("ipc_path", "/tmp/reth.ipc")?;
        settings.set_default("future_block_offset", 10)?;
        settings.set_default("state_file", "budget_system_state.json")?;
        settings.set_default("script_file", "input_script.json")?;
        settings.set_default("default_total_counted_seats", 7)?;
        settings.set_default("default_max_earner_seats", 5)?;
        settings.set_default("default_qualified_majority_threshold", 0.7)?;
        settings.set_default("counted_vote_points", 5)?;
        settings.set_default("uncounted_vote_points", 2)?;
        settings.set_default("telegram.chat_id", "")?;

        // Add in the current environment file
        // Default to 'development' env if unspecified
        settings.merge(File::with_name("config").required(false))?;

        // Add in settings from environment variables (with a prefix of APP)
        settings.merge(config::Environment::with_prefix("APP"))?;

        let mut config: Self = settings.try_into()?;
        
        // Expand the tilde in the state_file path
        if config.state_file.starts_with('~') {
            let home = dirs::home_dir().ok_or(ConfigError::Message("Unable to determine home directory".to_string()))?;
            config.state_file = home.join(config.state_file.strip_prefix("~/").unwrap_or(&config.state_file)).to_string_lossy().into_owned();
        }

        // Load the Telegram token from an environment variable
        config.telegram.token = env::var("TELEGRAM_BOT_TOKEN")
            .expect("TELEGRAM_BOT_TOKEN must be set");


        Ok(config)
    }
}

impl TryFrom<Config> for AppConfig {
    type Error = ConfigError;

    fn try_from(config: Config) -> Result<Self, Self::Error> {
        Ok(Self {
            ipc_path: config.get_string("ipc_path")?,
            future_block_offset: config.get_int("future_block_offset")? as u64,
            state_file: config.get_string("state_file")?,
            script_file: config.get_string("script_file")?,
            default_total_counted_seats: config.get_int("default_total_counted_seats")? as usize,
            default_max_earner_seats: config.get_int("default_max_earner_seats")? as usize,
            default_qualified_majority_threshold: config.get_float("default_qualified_majority_threshold")?,
            counted_vote_points: config.get_int("counted_vote_points")? as u32,
            uncounted_vote_points: config.get_int("uncounted_vote_points")? as u32,
            telegram: TelegramConfig {
                chat_id: config.get_string("telegram.chat_id")?,
                token: String::new(),
            }
        })
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ipc_path: "/tmp/reth.ipc".to_string(),
            future_block_offset: 10,
            state_file: "budget_system_state.json".to_string(),
            script_file: "input_script.json".to_string(),
            default_total_counted_seats: 7,
            default_max_earner_seats: 5,
            default_qualified_majority_threshold: 0.7,
            counted_vote_points: 5,
            uncounted_vote_points: 2,
            telegram: TelegramConfig {
                chat_id: String::new(),
                token: String::new(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_app_config_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.ipc_path, "/tmp/reth.ipc");
        assert_eq!(config.future_block_offset, 10);
        assert_eq!(config.state_file, "budget_system_state.json");
        assert_eq!(config.script_file, "input_script.json");
        assert_eq!(config.default_total_counted_seats, 7);
        assert_eq!(config.default_max_earner_seats, 5);
        assert_eq!(config.default_qualified_majority_threshold, 0.7);
        assert_eq!(config.counted_vote_points, 5);
        assert_eq!(config.uncounted_vote_points, 2);
    }

    #[test]
    fn test_app_config_from_env() {
        env::set_var("APP_IPC_PATH", "/custom/path.ipc");
        env::set_var("APP_FUTURE_BLOCK_OFFSET", "20");
        env::set_var("APP_STATE_FILE", "custom_state.json");
        env::set_var("TELEGRAM_BOT_TOKEN", "test_token");

        let config = AppConfig::new().unwrap();
        assert_eq!(config.ipc_path, "/custom/path.ipc");
        assert_eq!(config.future_block_offset, 20);
        assert_eq!(config.state_file, "custom_state.json");
        assert_eq!(config.telegram.token, "test_token");

        // Clean up environment variables
        env::remove_var("APP_IPC_PATH");
        env::remove_var("APP_FUTURE_BLOCK_OFFSET");
        env::remove_var("APP_STATE_FILE");
        env::remove_var("TELEGRAM_BOT_TOKEN");
    }
}