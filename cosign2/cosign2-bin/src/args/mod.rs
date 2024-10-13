use {clap::Parser, sec1::der::Decode, std::path::PathBuf};

mod cli;
mod config;

pub use config::Error as ConfigError;

/// Program arguments loaded from the CLI and config file.
#[derive(Debug, Clone)]
pub enum Args {
    /// Dump the header contents to stdout.
    Dump { input: PathBuf },
    /// Sign a firmware file.
    Sign {
        pubkey: Option<secp256k1::PublicKey>,
        secret: secp256k1::SecretKey,
        input: PathBuf,
        output: Output,
        version: Option<semver::Version>,
        target: Option<Target>,
        known_pubkeys: Vec<secp256k1::PublicKey>,
        developer: bool,
    },
}

#[derive(Debug, Clone)]
pub enum Output {
    InPlace,
    File(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Atsama5d27KeyOs,
}

pub fn args<I, T>(args: I) -> Result<Args, Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = cli::Args::try_parse_from(args).map_err(Error::Cli)?;
    match cli.command {
        cli::Command::Dump { input: file } => Ok(Args::Dump { input: file }),
        cli::Command::Sign {
            pubkey,
            secret,
            config,
            input,
            in_place,
            output,
            firmware_version,
            developer,
            target,
            known_pubkey: known_pubkeys,
        } => {
            // Load and validate the config.
            let config = config.map(|c| config::Config::load(&c)).transpose()?;
            let config_pubkey = config.as_ref().and_then(|config| config.pubkey.clone());
            let config_secret_path = config
                .as_ref()
                .and_then(|config| {
                    config.secret.as_ref().map(|secret| {
                        if !secret.is_absolute() {
                            return Err(Error::SecretPathNotAbsoluteInConfig(secret.clone()));
                        }
                        Ok(secret.clone())
                    })
                })
                .transpose()?;
            let config_known_pubkeys = config
                .as_ref()
                .and_then(|config| config.known_pubkeys.clone());
            let config_target = config.as_ref().and_then(|config| config.target.clone());

            // Reconcile the CLI and config arguments. Error if anything is specified both
            // on the CLI and in the config file.
            let pubkey = match (pubkey, config_pubkey) {
                (None, None) => None,
                (None, Some(pubkey)) => Some(pubkey),
                (Some(pubkey), None) => Some(pubkey),
                (Some(_), Some(_)) => return Err(Error::PubkeyInConfigAndCli),
            };
            let secret = match (secret, config_secret_path) {
                (None, None) => return Err(Error::SecretMissing),
                (None, Some(secret_path)) => secret_path,
                (Some(secret), None) => secret,
                (Some(_), Some(_)) => return Err(Error::SecretInConfigAndCli),
            };
            let known_pubkeys = match (known_pubkeys, config_known_pubkeys) {
                (None, None) => Vec::new(),
                (None, Some(known_pubkeys)) => known_pubkeys,
                (Some(known_pubkeys), None) => known_pubkeys,
                (Some(_), Some(_)) => return Err(Error::KnownPubkeysInConfigAndCli),
            };
            let target = match (target, config_target) {
                (None, None) => None,
                (None, Some(target)) => Some(target),
                (Some(target), None) => Some(target),
                (Some(_), Some(_)) => return Err(Error::TargetInConfigAndCli),
            };

            // Parse the arguments.
            let pubkey = pubkey
                .map(|p| {
                    secp256k1::PublicKey::from_slice(
                        &hex::decode(p).map_err(|_| Error::InvalidPubkeyHex)?,
                    )
                    .map_err(Error::InvalidPubkey)
                })
                .transpose()?;
            let pem = std::fs::read(secret).map_err(Error::ReadPemFile)?;
            let key = pem::parse(pem)?;
            if key.tag() != "EC PRIVATE KEY" {
                return Err(Error::InvalidPemTag(key.tag().to_string()));
            }
            let secret = sec1::EcPrivateKey::from_der(key.contents())
                .map_err(Error::ParseDerContent)?
                .private_key;
            let secret =
                secp256k1::SecretKey::from_slice(secret).map_err(Error::InvalidSecretKey)?;
            if in_place && output.is_some() {
                return Err(Error::InPlaceAndOutputSpecified);
            }
            let output = if in_place {
                Output::InPlace
            } else {
                Output::File(output.unwrap())
            };
            let target = target
                .map(|t| match t.as_str() {
                    "atsama5d27-keyos" => Ok(Target::Atsama5d27KeyOs),
                    _ => Err(Error::InvalidTarget(t)),
                })
                .transpose()?;
            let known_pubkeys = known_pubkeys
                .into_iter()
                .map(|known_pubkey| {
                    hex::decode(&known_pubkey)
                        .map_err(|_| Error::InvalidKnownPubkeyHex(known_pubkey.clone()))
                        .and_then(|p| {
                            secp256k1::PublicKey::from_slice(&p)
                                .map_err(|_| Error::InvalidKnownPubkey(known_pubkey.clone()))
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Args::Sign {
                pubkey,
                secret,
                input,
                output,
                version: firmware_version,
                target,
                developer,
                known_pubkeys,
            })
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Cli(clap::Error),
    Config(ConfigError),
    InPlaceAndOutputSpecified,
    InvalidKnownPubkey(String),
    InvalidKnownPubkeyHex(String),
    InvalidPemTag(String),
    InvalidPubkey(secp256k1::Error),
    InvalidPubkeyHex,
    InvalidSecretKey(secp256k1::Error),
    InvalidTarget(String),
    KnownPubkeysInConfigAndCli,
    ParseDerContent(sec1::der::Error),
    ParsePemFile(pem::PemError),
    PubkeyInConfigAndCli,
    ReadPemFile(std::io::Error),
    SecretInConfigAndCli,
    SecretMissing,
    SecretPathNotAbsoluteInConfig(PathBuf),
    TargetInConfigAndCli,
}

impl From<pem::PemError> for Error {
    fn from(e: pem::PemError) -> Self {
        Error::ParsePemFile(e)
    }
}

impl From<ConfigError> for Error {
    fn from(e: ConfigError) -> Self {
        Error::Config(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Cli(e) => write!(f, "{}", e.render().ansi()),
            Error::Config(e) => write!(f, "config error: {e}"),
            Error::InPlaceAndOutputSpecified => {
                write!(f, "cannot specify both --in-place and --output (-o)")
            }
            Error::InvalidKnownPubkey(pubkey) => {
                write!(f, r#"user specified invalid known public key: "{pubkey}""#)
            }
            Error::InvalidKnownPubkeyHex(pubkey) => {
                write!(
                    f,
                    r#"user specified invalid known public key hex: "{}""#,
                    pubkey
                )
            }
            Error::InvalidPemTag(tag) => {
                write!(f, r#"invalid PEM tag: "{tag}", expected "EC PRIVATE KEY""#)
            }
            Error::InvalidPubkey(e) => write!(f, "user specified invalid public key: {e}"),
            Error::InvalidPubkeyHex => write!(f, "user specified invalid public key hex"),
            Error::InvalidSecretKey(e) => write!(f, "user specified invalid secret key: {e}"),
            Error::InvalidTarget(target) => {
                write!(f, r#"user specified invalid target: "{target}""#)
            }
            Error::KnownPubkeysInConfigAndCli => {
                write!(f, "known pubkeys specified in both config and cli")
            }
            Error::ParseDerContent(e) => {
                write!(f, "failed to parse DER content inside PEM file: {e}")
            }
            Error::ParsePemFile(e) => write!(f, "invalid PEM file: {e}"),
            Error::PubkeyInConfigAndCli => write!(f, "pubkey specified in both config and cli"),
            Error::ReadPemFile(e) => write!(f, "failed to read PEM file: {e}"),
            Error::SecretInConfigAndCli => write!(f, "secret specified in both config and cli"),
            Error::SecretMissing => write!(f, "user did not specify a secret key"),
            Error::SecretPathNotAbsoluteInConfig(path) => {
                write!(
                    f,
                    r#"config error: secret key path is not absolute: "{}""#,
                    path.to_str().unwrap_or("<invalid path>")
                )
            }
            Error::TargetInConfigAndCli => write!(f, "target specified in both config and cli"),
        }
    }
}

impl std::error::Error for Error {}
