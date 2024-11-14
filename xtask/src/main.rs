use cargo_metadata::semver::Version;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};
use std::{env, fs};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

const FIRMWARE_VERSION: &str = "0.1.0";

#[cfg(target_os = "linux")]
const SRECORD_PATH: &str = "misc/srecord_linux";

#[cfg(target_os = "windows")]
const SRECORD_PATH: &str = "misc\\srecord_win";

#[cfg(target_os = "macos")]
const SRECORD_PATH: &str = "misc/srecord_mac";

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
    /// Softdevice - s112_nrf52_7.2.0_softdevice.hex
    /// Bootloader in release version ( Mpu uart pins - BAUD rate 115200)
    /// Memory protection ( no probe access and bootloader and SD MBR area protected )
    #[command(verbatim_doc_comment)]
    BuildFwImage,
    /// Build a full package image with SD, bootloader and application without:
    /// Flash protection
    /// UART mpu pins ( just console ones )
    #[command(verbatim_doc_comment)]
    BuildFwDebugImage,
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR")).ancestors().nth(1).unwrap().to_path_buf()
}

fn srecord() -> PathBuf {
    which::which("srec_cat").unwrap_or(project_root().join(SRECORD_PATH).join("srec_cat"))
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
        exit(0);
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
        exit(0);
    }

    tracing::info!("Removing package folder...");
    let status = Command::new("rm")
        .current_dir(project_root())
        .arg("-rf")
        .arg("BtPackage")
        .status()
        .expect("Running rm failed");
    if !status.success() {
        exit(0)
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
        exit(0);
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
        exit(0);
    }

    tracing::info!("Removing package folder...");
    let status = Command::new("rm")
        .current_dir(project_root())
        .arg("-rf")
        .arg("BtPackageDebug")
        .status()
        .expect("Running rm failed");
    if !status.success() {
        exit(0)
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
        panic!("Bootloader build failed");
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
        panic!("Bootloader hex generation failed");
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
        panic!("Bootloader build failed");
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
        panic!("Bootloader hex generation failed");
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
        panic!("Firmware build failed");
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
        panic!("Firmware build failed");
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
        panic!("Firmware build failed");
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
        // .arg("-q")
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
        panic!("cosign2.toml not found at project root");
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
        panic!("cosign2 failed");
    }
}

fn build_bt_package() {
    tracing::info!("Converting bin signed package to hex file with starting offset 0x19800");
    let status = Command::new(srecord().clone())
        .current_dir(project_root().join(SRECORD_PATH))
        .args([
            "../../BtPackage/BT_application_signed.bin",
            "-Binary",
            "-o",
            "../../BtPackage/BT_application_signed.hex",
            "-Intel",
        ])
        .status()
        .expect("Running Cargo failed");
    if !status.success() {
        panic!("Converting bin to hex failed");
    }

    let status = Command::new(srecord().clone())
        .current_dir(project_root().join(SRECORD_PATH))
        .args([
            "../../BtPackage/BT_application_signed.hex",
            "-Intel",
            "-offset",
            "0x19000",
            "-o",
            "../../BtPackage/BT_application_signed.hex",
            "-Intel",
        ])
        .status()
        .expect("Running Cargo failed");
    if !status.success() {
        panic!("Converting bin to hex failed");
    }

    tracing::info!("Merging softdevice bootloader and BT signed application in single hex");
    let status = Command::new(srecord().clone())
        .current_dir(project_root().join(SRECORD_PATH))
        .args([
            "../../BtPackage/BT_application_signed.hex",
            "-Intel",
            "../../BtPackage/bootloader.hex",
            "-Intel",
            "../s112_nrf52_7.2.0_softdevice.hex",
            "-Intel",
            "-o",
            "../../BtPackage/BTApp_Full_Image.hex",
            "-Intel",
        ])
        .status()
        .expect("Running Cargo failed");
    if !status.success() {
        panic!("Merging signed package failed");
    }

    tracing::info!("Removing single hex files");
    let status = Command::new("rm")
        .current_dir(project_root().join("BtPackage"))
        .arg("-rf")
        .arg("bootloader.hex")
        .arg("BT_application_signed.hex")
        .arg("BtApp.hex")
        .status()
        .expect("Running rm failed");
    if !status.success() {
        exit(0)
    }
}

fn build_bt_package_debug() {
    tracing::info!("Merging softdevice bootloader and BT signed application in single hex");
    let status = Command::new(srecord().clone())
        .current_dir(project_root().join(SRECORD_PATH))
        .args([
            "../../BtPackageDebug/BtappDebug.hex",
            "-Intel",
            "../../BtPackageDebug/bootloaderDebug.hex",
            "-Intel",
            "../s112_nrf52_7.2.0_softdevice.hex",
            "-Intel",
            "-o",
            "../../BtPackageDebug/BTApp_Full_Image_debug.hex",
            "-Intel",
        ])
        .status()
        .expect("Running Cargo failed");
    if !status.success() {
        panic!("Merging signed package failed");
    }

    tracing::info!("Removing single hex files");
    let status = Command::new("rm")
        .current_dir(project_root().join("BtPackageDebug"))
        .arg("-rf")
        .arg("bootloaderDebug.hex")
        .arg("BtappDebug.hex")
        .status()
        .expect("Running rm failed");
    if !status.success() {
        exit(0)
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
