use clap::{Parser, Subcommand};
use std::{env, fs};
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};
use cargo_metadata::semver::Version;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

const FIRMWARE_VERSION: &str = "0.1.0";

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct XtaskArgs {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
   
    /// Build a full flashable firmware image, combining the bootloader and the softdevice hex fi
    #[command(verbatim_doc_comment)]
    BuildFirmwareImage
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR")).ancestors().nth(1).unwrap().to_path_buf()
}

pub fn cargo() -> String { env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()) }

fn build_tools_check(){
    tracing::info!("Checking cargo binutils install state");
    let status = Command::new(cargo())
    .stdout(Stdio::null())
    .stderr(Stdio::null())        
    .current_dir(project_root().join("firmware"))
    .args(["objcopy", "--version"])
    .status()
    .expect("Running Cargo objcopy version failes");
    if !status.success() {
        tracing::info!("Cargo binutils are missing, to install do");
        tracing::info!("cargo install cargo-binutils");
        tracing::info!("rustup component add llvm-tools");
    
        exit(0);
    }

}

fn build_bt_bootloader(){
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
    .args(["objcopy", "--release", "--", "-O", "ihex", "../bootloader.hex"])
    .status()
    .expect("Running Cargo objcopy failed");
    if !status.success() {
        panic!("Bootloader hex generation failed");
    }
}


fn build_bt_firmware(){
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
            .args(["objcopy", "--release", "--", "-O", "ihex", "btapp.hex"])
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
    .args(["objcopy", "--release", "--", "--gap-fill","0xFF","--pad-to","0x25800", "-O", "binary", "btapp.bin"])
    .status()
    .expect("Running Cargo objcopy failed");
    if !status.success() {
        panic!("Firmware build failed");
    }
}

fn sign_bt_firmware(){
    let cosign2_config_path = project_root().join("cosign2.toml");
    let cosign2_config_path_str = cosign2_config_path.to_str().unwrap();

    if let Err(e) = fs::File::open(&cosign2_config_path) {
        tracing::info!("Cosign2 config not found at {cosign2_config_path_str}: {}", e);
        panic!("cosign2.toml not found at project root");
    }

    // Verify that cosign2 exists
    if Command::new("cosign2").stdout(Stdio::null()).stderr(Stdio::null()).spawn().is_err() {
        tracing::info!("Installing cosign2 bin...");
        let status = Command::new(cargo())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(project_root().join("cosign2/cosign2-bin"))
        .args(["install","--path",".","--bin","cosign2"])
        .status()
        .unwrap();
        if !status.success() {
            tracing::error!("Cosign2 install process failed");
            exit(0)
        }
    }

    let version = Version::parse(FIRMWARE_VERSION).expect("Wrong version format").to_string();

    let mut args = vec!["sign", "-i", "./firmware/btapp.bin","-c", "cosign2.toml","-o","FwSigned.bin"];
    args.extend_from_slice(&["--firmware-version", &version]);
    tracing::info!("{:?}",args);


    if !Command::new("cosign2")
    .stdout(Stdio::null())
    .current_dir(project_root())
    .args(&args)
    .status()
    .unwrap()
    .success() {
        panic!("cosign2 failed");
    }

}

fn build_bt_package(){
    tracing::info!("Converting bin signed package to hex file with starting offset 0x19000");
    let status = Command::new("srec_cat")
    .current_dir(project_root().join("misc"))
    .args(["../FwSigned.bin","-Binary","-o","../BtSignedPackage.hex","-Intel"])
            .status()
            .expect("Running Cargo failed");
            if !status.success() {
        panic!("Converting bin to hex failed");
            }

    let status = Command::new("srec_cat")
    .current_dir(project_root().join("misc"))
    .args(["../BtSignedPackage.hex","-Intel","-offset","0x19000","-o","../BtSignedPackage.hex","-Intel"])
            .status()
    .expect("Running Cargo failed");
            if !status.success() {
        panic!("Converting bin to hex failed");
            }

    tracing::info!("Merging softdevice bootloader and BT signed application in single hex");
            let status = Command::new("srec_cat")
            .current_dir(project_root().join("misc"))
    .args(["../BtSignedPackage.hex","-Intel","../bootloader.hex","-Intel","./s112_nrf52_7.2.0_softdevice.hex","-Intel","-o","../BtFwSignedFullImg.hex","-Intel"])
            .status()
            .expect("Running Cargo failed");
            if !status.success() {
        panic!("Merging signed package failed");
    }
            }


fn main() {

    env::set_var("RUST_LOG", "info");

    // Tracing using RUST_LOG
    tracing_subscriber::registry()
    .with(fmt::layer())
    .with(EnvFilter::from_default_env())
    .init();

    let args = XtaskArgs::parse();

    match args.command {

        Commands::BuildFirmwareImage => {
            
            build_bt_bootloader();
            build_bt_firmware();
            sign_bt_firmware();
            build_bt_package();
            
            // let version = version.to_string();

            // let combined_img_path_str = combined_image.to_str().unwrap();
            // println!("Signing combined image at `{combined_img_path_str}` with cosign2");
        }
    }
}
