use cargo_metadata::semver::Version;
use clap::{Parser, Subcommand};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};
use std::{env, fs};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

const FIRMWARE_VERSION: &str = "0.1.1";

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct XtaskArgs {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a full flashable firmware image with:
    /// SoftDevice - s112_nrf52_7.2.0_softdevice.hex
    /// Bootloader in release version (MPU UART pins - baud rate 460800)
    /// Memory protection (no probe access and bootloader and SD MBR area protected)
    #[command(verbatim_doc_comment)]
    BuildFwImage,

    /// Build a full package image with SD, bootloader and application without:
    /// Flash protection
    /// UART pins are redirected to the console at 115200 baud rate
    #[command(verbatim_doc_comment)]
    BuildFwDebugImage,
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR")).ancestors().nth(1).unwrap().to_path_buf()
}

pub fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn build_tools_check() {
    tracing::info!("BUILDING PRODUCTION PACKAGE");
    tracing::info!("Checking cargo binutils install state");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root())
        .args(["objcopy", "--version"])
        .status()
        .expect("Running Cargo objcopy version fails");
    if !status.success() {
        tracing::info!("Please install cargo binutils with these commands:");
        tracing::info!("cargo install cargo-binutils");
        tracing::info!("rustup component add llvm-tools");
        exit(-1);
    }

    tracing::info!("Cargo clean...");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root())
        .args(["clean", "-r", "-p", "firmware", "-p", "bootloader"])
        .status()
        .expect("Running Cargo clean fails");
    if !status.success() {
        tracing::info!("Cargo clean not working");
        exit(-1);
    }

    tracing::info!("Removing package folder...");
    let status = Command::new("rm")
        .current_dir(project_root())
        .arg("-rf")
        .arg("BtPackage")
        .status()
        .expect("Running rm failed");
    if !status.success() {
        tracing::error!("Removing package folder failed");
        exit(-1)
    }

    let build_dir = project_root().join("BtPackage");
    if !build_dir.exists() {
        fs::create_dir(build_dir).unwrap();
    }
}

fn build_tools_check_debug() {
    tracing::warn!("BUILDING DEBUG PACKAGE!!!");
    tracing::info!("Checking cargo binutils install state");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root())
        .args(["objcopy", "--version"])
        .status()
        .expect("Running Cargo objcopy version fails");
    if !status.success() {
        tracing::info!("Please install cargo binutils with these commands:");
        tracing::info!("cargo install cargo-binutils");
        tracing::info!("rustup component add llvm-tools");
        exit(-1);
    }

    tracing::info!("Cargo clean...");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root())
        .args(["clean", "-r", "-p", "firmware", "-p", "bootloader"])
        .status()
        .expect("Running Cargo clean fails");
    if !status.success() {
        tracing::info!("Cargo clean not working");
        exit(-1);
    }

    tracing::info!("Removing package folder...");
    let status = Command::new("rm")
        .current_dir(project_root())
        .arg("-rf")
        .arg("BtPackageDebug")
        .status()
        .expect("Running rm failed");
    if !status.success() {
        tracing::error!("Removing package folder failed");
        exit(-1)
    }

    let build_dir = project_root().join("BtPackageDebug");
    if !build_dir.exists() {
        fs::create_dir(build_dir).unwrap();
    }
}

fn build_bt_bootloader() {
    tracing::info!("Building bootloader....");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("bootloader"))
        .arg("build")
        .arg("-r")
        .arg("-q")
        .status()
        .expect("Running Cargo failed");
    if !status.success() {
        tracing::error!("Bootloader build failed");
        exit(-1);
    }

    tracing::info!("Generating bootloader hex file...");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("bootloader"))
        .args(["objcopy", "--release", "--", "-O", "ihex", "../BtPackage/bootloader.hex"])
        .status()
        .expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Bootloader hex generation failed");
        exit(-1);
    }
}

fn build_bt_bootloader_debug() {
    tracing::info!("Building debug bootloader....");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("bootloader"))
        .arg("build")
        .arg("-r")
        .arg("-q")
        .arg("--no-default-features")
        .arg("--features")
        .arg("debug")
        .status()
        .expect("Running Cargo failed");
    if !status.success() {
        tracing::error!("Bootloader build failed");
        exit(-1);
    }

    tracing::info!("Generating bootloader hex file...");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("bootloader"))
        .args([
            "objcopy",
            "--release",
            "--no-default-features",
            "--features",
            "debug",
            "--",
            "-O",
            "ihex",
            "../BtPackageDebug/bootloaderDebug.hex",
        ])
        .status()
        .expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Bootloader hex generation failed");
        exit(-1);
    }
}

fn build_bt_firmware() {
    tracing::info!("Building application...");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("firmware"))
        .arg("build")
        .arg("-r")
        .arg("-q")
        .status()
        .expect("Running Cargo failed");
    if !status.success() {
        tracing::error!("Firmware build failed");
        exit(-1);
    }

    tracing::info!("Creating BT application hex file");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("firmware"))
        .args(["objcopy", "--release", "--", "-O", "ihex", "../BtPackage/BtApp.hex"])
        .status()
        .expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Firmware build failed");
        exit(-1);
    }

    // Created a full populated flash image to avoid the signed fw is different from the slice to check.
    // We will always get the full slice of flash where app is flashed ( 0x19000 up to 0x25800 )
    // Then signing we will have from 0x19000 up to 0x19800 the cosign2 header.
    tracing::info!("Creating BT application bin file");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("firmware"))
        .args([
            "objcopy",
            "--release",
            "--",
            "--pad-to",
            "0x26000",
            "-O",
            "binary",
            "../BtPackage/BT_application.bin",
        ])
        .status()
        .expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Firmware build failed");
        exit(-1);
    }
}

fn build_bt_debug_firmware() {
    tracing::info!("Building debug application...");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("firmware"))
        .arg("build")
        .arg("-r")
        .arg("-q")
        .arg("--no-default-features")
        .arg("--features")
        .arg("debug")
        .status()
        .expect("Running Cargo failed");
    if !status.success() {
        panic!("Firmware build failed");
    }

    tracing::info!("Creating BT application hex file");
    let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("firmware"))
        .args([
            "objcopy",
            "--release",
            "--no-default-features",
            "--features",
            "debug",
            "--",
            "-O",
            "ihex",
            "../BtPackageDebug/BtappDebug.hex",
        ])
        .status()
        .expect("Running Cargo objcopy failed");
    if !status.success() {
        panic!("Firmware build failed");
    }
}

fn sign_bt_firmware() {
    let cosign2_config_path = project_root().join("cosign2.toml");
    let cosign2_config_path_str = cosign2_config_path.to_str().unwrap();

    if let Err(e) = fs::File::open(&cosign2_config_path) {
        tracing::info!("Cosign2 config not found at {cosign2_config_path_str}: {}", e);
        exit(-1);
    }

    // Verify that cosign2 exists
    if Command::new("cosign2").stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_err() {
        tracing::error!("unable to find cosign2 tool, please install it:");
        println!("   git clone https://github.com/Foundation-Devices/keyOS tmpkeyos");
        println!("   cargo install --path tmpkeyos/imports/cosign2/cosign2-bin --bin cosign2");
        println!("   rm -rf tmpkeyos");
        exit(-1);
    }

    let version = Version::parse(FIRMWARE_VERSION).expect("Wrong version format").to_string();

    let mut args = vec![
        "sign",
        "-i",
        "./BtPackage/BT_application.bin",
        "-c",
        "cosign2.toml",
        "-o",
        "./BtPackage/BT_application_signed.bin",
    ];
    args.extend_from_slice(&["--firmware-version", &version]);

    tracing::info!("Signing binary Bt application with Cosign2...");

    if !Command::new("cosign2")
        .stdout(Stdio::null())
        .current_dir(project_root())
        .args(&args)
        .status()
        .unwrap()
        .success()
    {
        tracing::error!("cosign2 failed");
        exit(-1);
    }
}

enum MergeableFile<P: AsRef<Path>> {
    IHex(P),
    Binary(P, u32),
}

fn merge_files<P: AsRef<Path>>(inputs: Vec<MergeableFile<P>>, output: P) {
    let mut records = vec![];

    inputs.into_iter().for_each(|file| {
        match file {
            MergeableFile::IHex(path) => {
                let mut file = fs::File::open(path).expect("unable to open input file");
                let mut data = String::new();
                file.read_to_string(&mut data).expect("unable to read the whole file");

                // wrap ihex::Reader on string
                let ihex = ihex::Reader::new(&data);

                let mut upper_address = 0u32;
                // iterate through ihex records
                for record in ihex {
                    let record = record.expect("error while parsing ihex file");
                    match record {
                        ihex::Record::StartSegmentAddress { cs, ip } => {
                            upper_address = ((cs as u32) << 4) + (ip as u32);
                        }
                        ihex::Record::StartLinearAddress(addr) => {
                            upper_address = addr << 16;
                        }
                        ihex::Record::ExtendedSegmentAddress(addr) => {
                            upper_address = (addr as u32) << 4;
                        }
                        ihex::Record::ExtendedLinearAddress(addr) => {
                            upper_address = (addr as u32) << 16;
                        }
                        ihex::Record::Data { offset, value } => {
                            let address = upper_address + (offset as u32);
                            records.push((address, value));
                        }
                        ihex::Record::EndOfFile => {
                            // nothing to do
                        }
                    }
                }
            }
            MergeableFile::Binary(path, global_offset) => {
                let mut file = fs::File::open(path).expect("unable to open input file");

                let mut data = vec![];
                file.read_to_end(&mut data).expect("unable to read the whole file");

                // fill records with slice of 32 bytes from data
                for (i, chunk) in data.chunks(32).enumerate() {
                    let address = global_offset + (i as u32) * 32;
                    records.push((address, chunk.to_vec()));
                }
            }
        }
    });

    // sort all records by addresses
    records.sort_by_key(|(addr, _)| *addr);

    // prepare records for output file
    let mut out_records = vec![];

    // get first record to store starting upper address
    let (addr, _) = records[0];
    let mut segment_upper_address = addr >> 16;
    out_records.push(ihex::Record::StartLinearAddress(segment_upper_address));

    // iterate through records and push them to output vector
    for (addr, value) in records.iter() {
        let upper = addr >> 16;

        // write extend linear address record if it has changed
        if upper != segment_upper_address {
            out_records.push(ihex::Record::ExtendedLinearAddress(upper as u16));
        }

        let offset = addr & 0xffff;
        out_records.push(ihex::Record::Data {
            offset: offset as u16,
            value: value.clone(),
        });
        segment_upper_address = upper;
    }

    // push EOF record
    out_records.push(ihex::Record::EndOfFile);

    // create ihex file
    let data = ihex::create_object_file_representation(&out_records).expect("error while create ihex object");

    // write output file
    let mut file = fs::File::create(output).expect("unable to create output file");
    file.write_all(data.as_bytes()).expect("unable to write ihex object to file");
}

fn build_bt_package() {
    tracing::info!("Merging softdevice, bootloader and BT signed application (with 0x19000 offset) in single hex");
    merge_files(
        vec![
            MergeableFile::Binary(project_root().join("BtPackage/BT_application_signed.bin"), 0x19000),
            MergeableFile::IHex(project_root().join("BtPackage/bootloader.hex")),
            MergeableFile::IHex(project_root().join("misc/s112_nrf52_7.2.0_softdevice.hex")),
        ],
        project_root().join("BtPackage/BTApp_Full_Image.hex"),
    );

    tracing::info!("Removing temporary files");
    let status = Command::new("rm")
        .current_dir(project_root().join("BtPackage"))
        .arg("-rf")
        .arg("bootloader.hex")
        .arg("BtApp.hex")
        .status()
        .expect("Running rm failed");
    if !status.success() {
        tracing::error!("Removing single hex files failed");
        exit(-1)
    }
}

fn build_bt_package_debug() {
    tracing::info!("Merging softdevice bootloader and BT signed application in single hex");
    merge_files(
        vec![
            MergeableFile::IHex(project_root().join("BtPackageDebug/BtappDebug.hex")),
            MergeableFile::IHex(project_root().join("BtPackageDebug/bootloaderDebug.hex")),
            MergeableFile::IHex(project_root().join("misc/s112_nrf52_7.2.0_softdevice.hex")),
        ],
        project_root().join("BtPackageDebug/BTApp_Full_Image_debug.hex"),
    );

    tracing::info!("Removing temporary files");
    let status = Command::new("rm")
        .current_dir(project_root().join("BtPackageDebug"))
        .arg("-rf")
        .arg("bootloaderDebug.hex")
        .arg("BtappDebug.hex")
        .status()
        .expect("Running rm failed");
    if !status.success() {
        tracing::error!("Removing single hex files failed");
        exit(-1)
    }
}

fn main() {
    // Adding some info tracing just for logging activity
    env::set_var("RUST_LOG", "info");

    // Tracing using RUST_LOG
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let args = XtaskArgs::parse();

    match args.command {
        Commands::BuildFwImage => {
            build_tools_check();
            build_bt_bootloader();
            build_bt_firmware();
            sign_bt_firmware();
            build_bt_package();
        }
        Commands::BuildFwDebugImage => {
            build_tools_check_debug();
            build_bt_bootloader_debug();
            build_bt_debug_firmware();
            build_bt_package_debug();
        }
    }
}
