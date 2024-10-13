use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub pubkey: Option<String>,
    pub secret: Option<PathBuf>,
    pub known_pubkeys: Option<Vec<String>>,
    pub target: Option<String>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, Error> {
        let config = std::fs::read_to_string(path)?;
        toml::from_str(&config).map_err(Into::into)
    }
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Toml(toml::de::Error),
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::Toml(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "failed to read config file: {e}"),
            Error::Toml(e) => write!(f, "config file format error in TOML: {e}"),
        }
    }
}

impl std::error::Error for Error {}
