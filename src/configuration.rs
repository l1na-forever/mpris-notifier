use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use thiserror::Error;

const CONFIGURATION_FILENAME: &str = "config.toml";
lazy_static! {
    static ref CONFIGURATION_PATH: String = format!(
        "{}{}",
        home::home_dir().unwrap().display(),
        "/.config/mpris-notifier/"
    );
}

#[derive(Debug, Error)]
pub enum ConfigurationError {
    #[error("error parsing configuration")]
    Parsing(#[from] toml::de::Error),
}
/// Configuration file used by mpris-notifier, located at
/// `$HOME/config/mpris-notifier/config.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Configuration {
    /// Format string for the notification subject text.
    ///
    /// Default: [DEFAULT_SUBJECT_FORMAT]
    pub subject_format: String,

    /// Format string for the notification message text.
    ///
    /// Default: [DEFAULT_BODY_FORMAT]
    pub body_format: String,

    /// For fields including multiple entities (such as "artists"), this
    /// determines which character is used to join the strings.
    ///
    /// Default: [DEFAULT_JOIN_STRING]
    pub join_string: String,

    /// Enable album artwork fetch. When enabled, album artwork will appear
    /// alongside the album art, provided that the art fetch completes within
    /// the deadline.
    ///
    /// Default: [DEFAULT_ENABLE_ALBUM_ART]
    pub enable_album_art: bool,

    /// The deadline, in milliseconds, before which the album art fetch must
    /// complete, else the notification will be sent without artwork.
    ///
    /// Default: [DEFAULT_ALBUM_ART_DEADLINE]
    pub album_art_deadline: u32,
}

const DEFAULT_SUBJECT_FORMAT: &str = "{track}";
const DEFAULT_BODY_FORMAT: &str = "{album} - {artist}";
const DEFAULT_JOIN_STRING: &str = ", ";
const DEFAULT_ENABLE_ALBUM_ART: bool = true;
const DEFAULT_ALBUM_ART_DEADLINE: u32 = 1000;

impl Default for Configuration {
    fn default() -> Self {
        Self {
            subject_format: DEFAULT_SUBJECT_FORMAT.to_string(),
            body_format: DEFAULT_BODY_FORMAT.to_string(),
            join_string: DEFAULT_JOIN_STRING.to_string(),
            enable_album_art: DEFAULT_ENABLE_ALBUM_ART,
            album_art_deadline: DEFAULT_ALBUM_ART_DEADLINE,
        }
    }
}

// Loads a configuration. If a configuration file is not found, one is created
// with default values, and the default values are used to start the program.
pub fn load_configuration() -> Result<Configuration, ConfigurationError> {
    // If we have an existing config file, try to load it and use that
    let full_path = format!("{}{}", *CONFIGURATION_PATH, CONFIGURATION_FILENAME);
    if let Ok(existing_toml) = fs::read_to_string(&full_path) {
        let config: Configuration = toml::from_str(&existing_toml)?;
        return Ok(config);
    }

    // Otherwise, attempt to create a default config file, and then use the
    // default values
    let default_config = Configuration::default();
    if let Err(err) = fs::create_dir_all(&*CONFIGURATION_PATH) {
        log::warn!(
            "Unable to create configuration directory `{}`, using defaults: {}",
            *CONFIGURATION_PATH,
            err
        );
        return Ok(default_config);
    }

    let default_toml = toml::to_string_pretty(&default_config).unwrap();
    if let Err(err) = fs::write(&full_path, default_toml) {
        log::warn!(
            "Unable to write default configuration file `{}`, using defaults: {}",
            &full_path,
            err
        );
    }
    Ok(default_config)
}
