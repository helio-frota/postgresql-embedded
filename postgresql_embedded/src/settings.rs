use crate::error::{Error, Result};
use home::home_dir;
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::collections::HashMap;
use std::env;
use std::env::current_dir;
use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use url::Url;

/// PostgreSQL's superuser
pub const BOOTSTRAP_SUPERUSER: &str = "postgres";

/// Database settings
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct Settings {
    /// PostgreSQL's installation directory
    pub installation_dir: PathBuf,
    /// PostgreSQL password file
    pub password_file: PathBuf,
    /// PostgreSQL data directory
    pub data_dir: PathBuf,
    /// PostgreSQL host
    pub host: String,
    /// PostgreSQL port
    pub port: u16,
    /// PostgreSQL user name
    pub username: String,
    /// PostgreSQL password
    pub password: String,
    /// Temporary database
    pub temporary: bool,
    /// Command execution Timeout
    pub timeout: Option<Duration>,
}

/// Settings implementation
impl Settings {
    /// Create a new instance of [`Settings`]
    pub fn new() -> Self {
        let home_dir = home_dir().unwrap_or_else(|| env::current_dir().unwrap_or_default());
        let passwword_file_name = ".pgpass";
        let password_file = match tempfile::tempdir() {
            Ok(dir) => dir.into_path().join(passwword_file_name),
            Err(_) => {
                let current_dir = current_dir().unwrap_or(PathBuf::from("."));
                current_dir.join(passwword_file_name)
            }
        };
        let data_dir = match tempfile::tempdir() {
            Ok(dir) => dir.into_path(),
            Err(_) => {
                let temp_dir: String = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(16)
                    .map(char::from)
                    .collect();

                let data_dir = current_dir().unwrap_or(PathBuf::from("."));
                data_dir.join(temp_dir)
            }
        };
        let password = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();

        Self {
            installation_dir: home_dir.join(".theseus").join("postgresql"),
            password_file,
            data_dir,
            host: "localhost".to_string(),
            port: 0,
            username: BOOTSTRAP_SUPERUSER.to_string(),
            password,
            temporary: true,
            timeout: Some(Duration::from_secs(5)),
        }
    }

    /// Returns the binary directory for the configured PostgreSQL installation.
    pub fn binary_dir(&self) -> PathBuf {
        self.installation_dir.join("bin")
    }

    /// Return the PostgreSQL URL for the given database name.
    pub fn url<S: AsRef<str>>(&self, database_name: S) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            self.username,
            self.password,
            self.host,
            self.port,
            database_name.as_ref()
        )
    }

    /// Create a new instance of [`Settings`] from the given URL.
    pub fn from_url<S: AsRef<str>>(url: S) -> Result<Self> {
        let parsed_url = match Url::parse(url.as_ref()) {
            Ok(parsed_url) => parsed_url,
            Err(error) => {
                return Err(Error::InvalidUrl {
                    url: url.as_ref().to_string(),
                    message: error.to_string(),
                });
            }
        };
        let query_parameters: HashMap<String, String> =
            parsed_url.query_pairs().into_owned().collect();
        let mut settings = Self::default();

        if !parsed_url.username().is_empty() {
            settings.username = parsed_url.username().to_string();
        }
        if let Some(password) = parsed_url.password() {
            settings.password = password.to_string();
        }
        if let Some(host) = parsed_url.host() {
            settings.host = host.to_string();
        }
        if let Some(port) = parsed_url.port() {
            settings.port = port;
        }
        if let Some(installation_dir) = query_parameters.get("installation_dir") {
            if let Ok(path) = PathBuf::from_str(installation_dir) {
                settings.installation_dir = path;
            }
        }
        if let Some(password_file) = query_parameters.get("password_file") {
            if let Ok(path) = PathBuf::from_str(password_file) {
                settings.password_file = path;
            }
        }
        if let Some(data_dir) = query_parameters.get("data_dir") {
            if let Ok(path) = PathBuf::from_str(data_dir) {
                settings.data_dir = path;
            }
        }
        if let Some(temporary) = query_parameters.get("temporary") {
            settings.temporary = temporary == "true";
        }
        if let Some(timeout) = query_parameters.get("timeout") {
            settings.timeout = match timeout.parse::<u64>() {
                Ok(timeout) => Some(Duration::from_secs(timeout)),
                Err(error) => {
                    return Err(Error::InvalidUrl {
                        url: url.as_ref().to_string(),
                        message: error.to_string(),
                    });
                }
            };
        }

        Ok(settings)
    }
}

/// Implement the [`Settings`] trait for [`Settings`]
impl postgresql_commands::Settings for Settings {
    fn get_binary_dir(&self) -> PathBuf {
        self.binary_dir().clone()
    }

    fn get_host(&self) -> OsString {
        self.host.parse().expect("host")
    }

    fn get_port(&self) -> u16 {
        self.port
    }

    fn get_username(&self) -> OsString {
        self.username.parse().expect("username")
    }

    fn get_password(&self) -> OsString {
        self.password.parse().expect("password")
    }
}

/// Default implementation for [`Settings`]
impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    fn test_settings_new() -> Result<()> {
        let settings = Settings::new();
        assert!(!settings
            .installation_dir
            .to_str()
            .unwrap_or_default()
            .is_empty());
        assert!(settings.password_file.ends_with(".pgpass"));
        assert!(!settings.data_dir.to_str().unwrap_or_default().is_empty());
        assert_eq!(0, settings.port);
        assert_eq!(BOOTSTRAP_SUPERUSER, settings.username);
        assert!(!settings.password.is_empty());
        assert_ne!("password", settings.password);
        assert!(settings.binary_dir().ends_with("bin"));
        assert_eq!(
            "postgresql://postgres:password@localhost:0/test",
            settings
                .url("test")
                .replace(settings.password.as_str(), "password")
        );
        assert_eq!(Some(Duration::from_secs(5)), settings.timeout);
        Ok(())
    }

    #[test]
    fn test_settings_from_url() -> Result<()> {
        let base_url = "postgresql://postgres:password@localhost:5432/test";
        let installation_dir = "installation_dir=/tmp/postgresql";
        let password_file = "password_file=/tmp/.pgpass";
        let data_dir = "data_dir=/tmp/data";
        let temporary = "temporary=false";
        let timeout = "timeout=10";
        let url = format!("{base_url}?{installation_dir}&{password_file}&{data_dir}&{temporary}&{temporary}&{timeout}");

        let settings = Settings::from_url(url)?;

        assert_eq!(BOOTSTRAP_SUPERUSER, settings.username);
        assert_eq!("password", settings.password);
        assert_eq!("localhost", settings.host);
        assert_eq!(5432, settings.port);
        assert_eq!(base_url, settings.url("test"));
        assert_eq!(PathBuf::from("/tmp/postgresql"), settings.installation_dir);
        assert_eq!(PathBuf::from("/tmp/.pgpass"), settings.password_file);
        assert_eq!(PathBuf::from("/tmp/data"), settings.data_dir);
        assert!(!settings.temporary);
        assert_eq!(Some(Duration::from_secs(10)), settings.timeout);

        Ok(())
    }

    #[test]
    fn test_settings_from_url_invalid_url() {
        assert!(Settings::from_url("^`~").is_err());
    }

    #[test]
    fn test_settings_from_url_invalid_timeout() {
        assert!(Settings::from_url("postgresql://?timeout=foo").is_err());
    }
}
