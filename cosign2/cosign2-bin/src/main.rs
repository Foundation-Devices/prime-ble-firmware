use {
    colored::Colorize,
    cosign2::VerificationResult,
    sha2::Digest,
    std::{
        ffi::OsString,
        io::{Read, Seek, Write},
        path::{Path, PathBuf},
    },
};

mod args;

#[cfg(test)]
mod tests;

fn main() -> std::process::ExitCode {
    main_args(
        std::env::args_os(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .into()
}

fn main_args<I, T>(args: I, stdout: impl Write, mut stderr: impl Write) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match run(args, stdout) {
        Ok(()) => ExitCode(0),
        Err(Error::Args(e @ args::Error::Cli(_))) => {
            // Clap already does the "error: {}" formatting.
            writeln!(stderr, "{e}").expect("write error to stderr");
            ExitCode(1)
        }
        Err(e) => {
            writeln!(stderr, "{} {e}", "error:".bold().red()).expect("write error to stderr");
            ExitCode(1)
        }
    }
}

fn run<I, T>(args: I, mut stdout: impl Write) -> Result<(), Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match args::args(args)? {
        args::Args::Dump { input } => {
            let mut input_buf = Vec::new();
            std::fs::File::open(input)
                .map_err(Error::OpenInputFile)?
                .read_to_end(&mut input_buf)
                .map_err(Error::ReadInputFile)?;
            match cosign2::Header::parse(&input_buf, &[], &Sha256, &Secp256k1Verify::default())? {
                Some(header) => {
                    let magic = match header.magic() {
                        cosign2::Magic::Atsama5d27KeyOs => "atsama5d27-keyos",
                    };
                    let timestamp = chrono::DateTime::from_timestamp(header.timestamp().into(), 0)
                        .expect("valid timestamp");
                    writeln!(&mut stdout, "{:10} {magic}", "magic".bold())
                        .map_err(Error::Stdout)?;
                    writeln!(
                        &mut stdout,
                        "{:10} {timestamp} ({})",
                        "timestamp".bold(),
                        header.timestamp(),
                    )
                    .map_err(Error::Stdout)?;
                    writeln!(&mut stdout, "{:10} {}", "date".bold(), header.date())
                        .map_err(Error::Stdout)?;
                    writeln!(&mut stdout, "{:10} {}", "version".bold(), header.version())
                        .map_err(Error::Stdout)?;
                    let human_size =
                        humansize::format_size(header.firmware_size(), humansize::BINARY);
                    writeln!(
                        &mut stdout,
                        "{:10} {human_size} ({})",
                        "size".bold(),
                        header.firmware_size(),
                    )
                    .map_err(Error::Stdout)?;
                    writeln!(
                        &mut stdout,
                        "{:10} {}",
                        "pubkey1".bold(),
                        hex::encode(header.pubkey1()),
                    )
                    .map_err(Error::Stdout)?;
                    let signature1 = header.signature1();
                    writeln!(
                        &mut stdout,
                        "{:10} {}",
                        "signature1".bold(),
                        hex::encode(&signature1[..32]),
                    )
                    .map_err(Error::Stdout)?;
                    writeln!(
                        &mut stdout,
                        "{} {}",
                        " ".repeat(10),
                        hex::encode(&signature1[32..]),
                    )
                    .map_err(Error::Stdout)?;
                    writeln!(
                        &mut stdout,
                        "{:10} {}",
                        "pubkey2".bold(),
                        hex::encode(header.pubkey2()),
                    )
                    .map_err(Error::Stdout)?;
                    let signature2 = header.signature2();
                    writeln!(
                        &mut stdout,
                        "{:10} {}",
                        "signature2".bold(),
                        hex::encode(&signature2[..32]),
                    )
                    .map_err(Error::Stdout)?;
                    writeln!(
                        &mut stdout,
                        "{} {}",
                        " ".repeat(10),
                        hex::encode(&signature2[32..]),
                    )
                    .map_err(Error::Stdout)?;
                }
                None => {
                    writeln!(&mut stdout, "{}", "no header found".bold()).map_err(Error::Stdout)?
                }
            }
        }
        args::Args::Sign {
            pubkey: expected_pubkey,
            secret,
            input: input_path,
            output,
            version,
            target,
            known_pubkeys,
            developer,
        } => {
            // Check that user is not accidentally signing with the wrong key.
            let pubkey = secret.public_key(&secp256k1::Secp256k1::new());
            if let Some(expected_pubkey) = expected_pubkey {
                if pubkey != expected_pubkey {
                    return Err(Error::KeyMismatch);
                }
            }
            if !known_pubkeys.is_empty() && !known_pubkeys.contains(&pubkey) {
                return Err(Error::UnknownSigner);
            }

            let known_pubkeys: Vec<_> = known_pubkeys.into_iter().map(|k| k.serialize()).collect();

            let magic = target.map(|t| match t {
                args::Target::Atsama5d27KeyOs => cosign2::Magic::Atsama5d27KeyOs,
            });
            let mut input_options = std::fs::OpenOptions::new();
            input_options.read(true);
            if let args::Output::InPlace = output {
                // When working in-place, the input file will be written to.
                input_options.write(true);
            }
            let mut input = input_options
                .open(&input_path)
                .map_err(Error::OpenInputFile)?;
            let output = match output {
                args::Output::InPlace => OutputFile::InPlace(
                    // When working in-place, first write to a temporary file. The temporary
                    // file will be moved to the input file at the end of the process.
                    tempfile::NamedTempFile::new().map_err(Error::CreateTempFile)?,
                ),
                args::Output::File(path) => {
                    let mut output_options = std::fs::OpenOptions::new();
                    let file = output_options
                        // After being written, the output file will be read and parsed as a sanity
                        // check.
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&path)
                        .map_err(Error::OpenOutputFile)?;
                    OutputFile::File(file, path.to_owned())
                }
            };

            let mut input_buf = Vec::new();
            input
                .read_to_end(&mut input_buf)
                .map_err(Error::ReadInputFile)?;

            let (header, mut firmware) = match cosign2::Header::parse(
                &input_buf,
                known_pubkeys.as_slice(),
                &Sha256,
                &Secp256k1Verify::default(),
            )? {
                // If a header is present, add a second signature.
                Some(mut header) => {
                    match version {
                        Some(version) if header.version() != version.to_string() => {
                            return Err(Error::VersionMismatch {
                                header: header.version().to_owned(),
                                expected: version.to_string(),
                            });
                        }
                        _ => {}
                    }
                    match magic {
                        Some(magic) if header.magic() != magic => {
                            return Err(Error::MagicMismatch {
                                header: header.magic(),
                                expected: magic,
                            });
                        }
                        _ => {}
                    }
                    header.add_second_signature(&Secp256k1Sign::new(secret))?;
                    (header, &input_buf[cosign2::Header::SIZE..])
                }
                // If no header is present, create and sign a new header.
                None => {
                    let header = cosign2::Header::sign_new(
                        magic.ok_or(Error::TargetMissing)?,
                        &version.ok_or(Error::VersionMissing)?.to_string(),
                        chrono::Utc::now().timestamp().try_into().expect("not 2106"),
                        if developer {
                            cosign2::Signer::Developer
                        } else {
                            cosign2::Signer::Trusted
                        },
                        &input_buf,
                        &Sha256,
                        &Secp256k1Sign::new(secret),
                    )?;
                    (header, input_buf.as_slice())
                }
            };

            // Write the signed header and firmware to output file.
            let mut header_buf = [0u8; cosign2::Header::SIZE];
            header.serialize(&mut header_buf)?;
            output
                .file()
                .write_all(&header_buf)
                .map_err(Error::WriteOutputFile)?;
            std::io::copy(&mut firmware, &mut output.file()).map_err(Error::WriteOutputFile)?;

            // When working in-place, the output is actually first written to a temporary
            // file. To achieve the effect of in-place editing, the temporary
            // file must be moved to the input file, replacing it.
            if output.is_in_place() {
                move_file(output.file(), output.path(), &input, &input_path)
                    .map_err(Error::MoveTempFile)?;
            }

            // Sanity check that the output file can be parsed. If not, it's possible the
            // output file was being used by another process.
            output
                .file()
                .seek(std::io::SeekFrom::Start(0))
                .map_err(Error::SeekOutputFile)?;
            let mut output_buf = Vec::new();
            output
                .file()
                .read_to_end(&mut output_buf)
                .map_err(Error::ReadOutputFile)?;
            cosign2::Header::parse(
                &output_buf,
                &known_pubkeys,
                &Sha256,
                &Secp256k1Verify::default(),
            )
            .map_err(Error::ParseOutputFile)?
            .ok_or(Error::ParseOutputFileNoHeader)?;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Secp256k1Sign {
    secp256k1: secp256k1::Secp256k1<secp256k1::All>,
    key: secp256k1::SecretKey,
}

impl Secp256k1Sign {
    fn new(key: secp256k1::SecretKey) -> Self {
        Self {
            secp256k1: secp256k1::Secp256k1::new(),
            key,
        }
    }
}

impl cosign2::Secp256k1Sign for Secp256k1Sign {
    fn sign_ecdsa(&self, msg: [u8; 32]) -> [u8; 64] {
        self.secp256k1
            .sign_ecdsa(&secp256k1::Message::from_digest(msg), &self.key)
            .serialize_compact()
    }

    fn pubkey(&self) -> [u8; 33] {
        self.key.public_key(&self.secp256k1).serialize()
    }
}

#[derive(Debug, Default)]
struct Secp256k1Verify(secp256k1::Secp256k1<secp256k1::All>);

impl cosign2::Secp256k1Verify for Secp256k1Verify {
    fn verify_ecdsa(
        &self,
        msg: [u8; 32],
        signature: [u8; 64],
        pubkey: [u8; 33],
    ) -> VerificationResult {
        let Ok(pubkey) = secp256k1::PublicKey::from_slice(&pubkey) else {
            return VerificationResult::Invalid;
        };

        if self
            .0
            .verify_ecdsa(
                &secp256k1::Message::from_digest(msg),
                &secp256k1::ecdsa::Signature::from_compact(&signature).expect("64 bytes"),
                &pubkey,
            )
            .is_ok()
        {
            VerificationResult::Valid
        } else {
            VerificationResult::Invalid
        }
    }
}

#[derive(Debug, Default)]
struct Sha256;

impl cosign2::Sha256 for Sha256 {
    fn hash(&self, data: &[u8]) -> [u8; 32] {
        sha2::Sha256::digest(data).into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExitCode(u8);

impl From<ExitCode> for std::process::ExitCode {
    fn from(code: ExitCode) -> Self {
        code.0.into()
    }
}

#[derive(Debug)]
enum OutputFile {
    InPlace(tempfile::NamedTempFile),
    File(std::fs::File, PathBuf),
}

impl OutputFile {
    fn file(&self) -> &std::fs::File {
        match self {
            OutputFile::InPlace(file) => file.as_file(),
            OutputFile::File(file, _) => file,
        }
    }

    fn path(&self) -> &Path {
        match self {
            OutputFile::InPlace(file) => file.path(),
            OutputFile::File(_, path) => path,
        }
    }

    fn is_in_place(&self) -> bool {
        matches!(self, OutputFile::InPlace(_))
    }
}

fn move_file(
    mut from_file: &std::fs::File,
    from_path: &Path,
    mut to_file: &std::fs::File,
    to_path: &Path,
) -> std::io::Result<()> {
    if std::fs::rename(from_path, to_path).is_err() {
        // If rename fails, the files might be on different filesystems.
        // Fall back to copying and removing the original file.
        from_file.seek(std::io::SeekFrom::Start(0))?;
        to_file.seek(std::io::SeekFrom::Start(0))?;
        std::io::copy(&mut from_file, &mut to_file)?;
        std::fs::remove_file(from_path)?;
    }
    Ok(())
}

#[derive(Debug)]
enum Error {
    Args(args::Error),
    Cosign2(cosign2::Error),
    CreateTempFile(std::io::Error),
    KeyMismatch,
    MagicMismatch {
        header: cosign2::Magic,
        expected: cosign2::Magic,
    },
    MoveTempFile(std::io::Error),
    OpenInputFile(std::io::Error),
    OpenOutputFile(std::io::Error),
    ParseOutputFile(cosign2::Error),
    ParseOutputFileNoHeader,
    ReadInputFile(std::io::Error),
    ReadOutputFile(std::io::Error),
    SeekOutputFile(std::io::Error),
    Stdout(std::io::Error),
    TargetMissing,
    UnknownSigner,
    VersionMismatch {
        header: String,
        expected: String,
    },
    VersionMissing,
    WriteOutputFile(std::io::Error),
}

impl From<args::Error> for Error {
    fn from(e: args::Error) -> Self {
        Error::Args(e)
    }
}

impl From<cosign2::Error> for Error {
    fn from(e: cosign2::Error) -> Self {
        Error::Cosign2(e)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Args(e) => write!(f, "{e}"),
            Error::Cosign2(e) => write!(f, "{e}"),
            Error::CreateTempFile(e) => write!(f, "failed to create temporary file: {e}"),
            Error::KeyMismatch => write!(f, "public key does not match secret key"),
            Error::MagicMismatch { header, expected } => {
                write!(
                    f,
                    "unexpected magic in header {}, expected {}",
                    hex::encode(header.to_bytes()),
                    hex::encode(expected.to_bytes())
                )
            }
            Error::MoveTempFile(e) => write!(f, "failed to move temporary file to output: {e}"),
            Error::OpenInputFile(e) => write!(f, "failed to open input file: {e}"),
            Error::OpenOutputFile(e) => write!(f, "failed to open output file: {e}"),
            Error::ParseOutputFile(e) => write!(
                f,
                "failed to parse output file after writing: {e}; is another process using it?"
            ),
            Error::ParseOutputFileNoHeader => write!(
                f,
                "output file has no header after writing; is another process using it?"
            ),
            Error::ReadInputFile(e) => write!(f, "failed to read input file: {e}"),
            Error::ReadOutputFile(e) => write!(f, "failed to read output file: {e}"),
            Error::SeekOutputFile(e) => write!(f, "failed to seek output file: {e}"),
            Error::Stdout(e) => write!(f, "failed to write to stdout: {e}"),
            Error::TargetMissing => write!(
                f,
                "target must be specified, either with --target or in the config file"
            ),
            Error::UnknownSigner => write!(f, "signing key is not in known pubkeys list"),
            Error::VersionMismatch { header, expected } => {
                write!(
                    f,
                    "version in header ({header}) does not match --firmware-version ({expected}), \
                     omit the flag or specify the correct version"
                )
            }
            Error::VersionMissing => {
                write!(
                    f,
                    "--firmware-version must be specified when signing a file without a header"
                )
            }
            Error::WriteOutputFile(e) => write!(f, "failed to write to output file: {e}"),
        }
    }
}

impl std::error::Error for Error {}
