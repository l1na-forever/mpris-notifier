use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use thiserror::Error;

const CONFIGURATION_FILENAME: &str = "config.toml";

lazy_static! {
    pub static ref CONFIGURATION_PATH: String = format!(
        "{}{}",
        home::home_dir().unwrap().display(),
        "/.config/mpris-notifier/"
    );
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigurationError {
    #[error("error parsing configuration")]
    Parsing(#[from] toml::de::Error),
}

/// Configuration file used by mpris-notifier, located at
/// `$HOME/config/mpris-notifier/config.toml`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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

    /// A list of commands to be called on each notification. Each command
    /// should be given as a sequence, the first item being the program and
    /// following items being arguments.
    ///
    /// Default: [DEFAULT_COMMANDS]
    pub commands: Vec<Vec<String>>,
}

const DEFAULT_SUBJECT_FORMAT: &str = "{track}";
const DEFAULT_BODY_FORMAT: &str = "{album} - {artist}";
const DEFAULT_JOIN_STRING: &str = ", ";
const DEFAULT_ENABLE_ALBUM_ART: bool = true;
const DEFAULT_ALBUM_ART_DEADLINE: u32 = 1000;
const DEFAULT_COMMANDS: Vec<Vec<String>> = vec![];

impl Default for Configuration {
    fn default() -> Self {
        Self {
            subject_format: DEFAULT_SUBJECT_FORMAT.to_string(),
            body_format: DEFAULT_BODY_FORMAT.to_string(),
            join_string: DEFAULT_JOIN_STRING.to_string(),
            enable_album_art: DEFAULT_ENABLE_ALBUM_ART,
            album_art_deadline: DEFAULT_ALBUM_ART_DEADLINE,
            commands: DEFAULT_COMMANDS,
        }
    }
}

pub fn load_configuration() -> Result<Configuration, ConfigurationError> {
    let full_path = format!("{}{}", *CONFIGURATION_PATH, CONFIGURATION_FILENAME);
    load_configuration_from_path(&full_path)
}

// Loads a configuration. If a configuration file is not found, one is created
// with default values, and the default values are used to start the program.
fn load_configuration_from_path(full_path: &str) -> Result<Configuration, ConfigurationError> {
    // If we have an existing config file, try to load it and use that
    if let Ok(existing_toml) = fs::read_to_string(full_path) {
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
    if let Err(err) = fs::write(full_path, default_toml) {
        log::warn!(
            "Unable to write default configuration file `{}`, using defaults: {}",
            &full_path,
            err
        );
    }
    Ok(default_config)
}

#[cfg(test)]
mod tests {
    use crate::configuration::{load_configuration_from_path, ConfigurationError};
    use crate::Configuration;
    use lazy_static::lazy_static;
    use std::{fs, matches};
    use tempfile::TempDir;

    lazy_static! {
        static ref TEST_TEMP_DIR: String =
            TempDir::new().unwrap().into_path().display().to_string();
    }

    #[test]
    fn test_load_configuration_existing_happy() {
        let conf_path = format!("{}{}", &*TEST_TEMP_DIR, "happy.toml");
        let conf_data = r#"subject_format = '{track}'
                          body_format = "{album}\n{artist}"
                          join_string = ' ⬥ '
                          enable_album_art = true
                          album_art_deadline = 1500
                          commands = [['pkill', '-RTMIN+2', 'waybar'], ['~/script.sh']]"#;
        let expected = Configuration {
            subject_format: "{track}".to_string(),
            body_format: "{album}\n{artist}".to_string(),
            join_string: " ⬥ ".to_string(),
            enable_album_art: true,
            album_art_deadline: 1500,
            commands: vec![
                vec![
                    "pkill".to_string(),
                    "-RTMIN+2".to_string(),
                    "waybar".to_string(),
                ],
                vec!["~/script.sh".to_string()],
            ],
        };
        fs::create_dir_all(&*TEST_TEMP_DIR).expect("test setup failed");
        fs::write(&conf_path, conf_data).expect("test setup failed");

        let result =
            load_configuration_from_path(&conf_path).expect("expected valid configuration to load");
        assert_eq!(expected, result);
    }

    #[test]
    fn test_load_configuration_existing_invalid() {
        let conf_path = format!("{}{}", &*TEST_TEMP_DIR, "invalid.toml");
        let conf_data = "?????";
        fs::create_dir_all(&*TEST_TEMP_DIR).expect("test setup failed");
        fs::write(&conf_path, conf_data).expect("test setup failed");

        let err = load_configuration_from_path(&conf_path)
            .expect_err("expected invalid configuration to fail to load");
        assert!(matches!(err, ConfigurationError::Parsing(_)));
    }

    #[test]
    fn test_load_configuration_default_fail_write_default() {
        let mut permissions = fs::metadata(&*TEST_TEMP_DIR).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&*TEST_TEMP_DIR, permissions).expect("test setup failed");
        let result = load_configuration_from_path(&TEST_TEMP_DIR)
            .expect("default config should be returned even if writing one fails");

        assert_eq!(Configuration::default(), result);
    }

    #[test]
    fn test_load_configuration_default_happy() {
        let path = format!("{}{}", &*TEST_TEMP_DIR, "404");
        let result = load_configuration_from_path(&path)
            .expect("missing configuration should load a default");

        assert_eq!(Configuration::default(), result);
    }
}
