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

        // Add in the current environment file
        // Default to 'development' env if unspecified
        settings.merge(File::with_name("config").required(false))?;

        // Add in settings from environment variables (with a prefix of APP)
        settings.merge(config::Environment::with_prefix("APP"))?;

        // You can add more sources here if needed, like command-line arguments

        settings.try_into()
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
        }
    }
}