// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

use cargo_metadata::MetadataCommand;
use clap::{Parser, Subcommand};
use consts::{BASE_APP_ADDR, BASE_BOOTLOADER_ADDR, SIGNATURE_HEADER_SIZE};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Stdio};
use std::{env, fs};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct XtaskArgs {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a full flashable firmware image with:
    /// SoftDevice
    /// Bootloader in release version
    /// Memory protection (no probe access and bootloader and SD MBR area protected)
    #[command(verbatim_doc_comment)]
    BuildFwImage,

    /// Build a minimal flashable firmware image with:
    /// SoftDevice
    /// Bootloader in release version
    /// Memory protection (no probe access and bootloader and SD MBR area protected)
    #[command(verbatim_doc_comment)]
    BuildMinimalImage,

    /// Build a full package image with SD, bootloader and application without:
    /// Flash protection
    #[command(verbatim_doc_comment)]
    BuildFwDebugImage,

    /// Patch SoftDevice hex file to save some space at the end of it
    #[command(verbatim_doc_comment)]
    PatchSd,

    /// Build firmware without signing or packaging
    BuildUnsigned,

    /// Sign the firmware with the provided cosign2 config
    SignFirmware {
        /// Path to cosign2.toml config file
        #[arg(default_value = "cosign2.toml")]
        config_path: String,
    },

    /// Package the signed firmware
    PackageFirmware,
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR")).ancestors().nth(1).unwrap().to_path_buf()
}

pub fn cargo() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
}

fn build_tools_check(verbose: bool) {
    tracing::info!("BUILDING PRODUCTION PACKAGE");
    tracing::info!("Checking cargo binutils install state");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root()).args(["objcopy", "--version"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy version fails");
    if !status.success() {
        tracing::info!("Please install cargo binutils with these commands:");
        tracing::info!("cargo install cargo-binutils");
        tracing::info!("rustup component add llvm-tools");
        exit(-1);
    }

    tracing::info!("Cargo clean...");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd
        .current_dir(project_root())
        .args(["clean", "--release", "-p", "firmware", "-p", "bootloader"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo clean fails");
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

fn build_tools_check_debug(verbose: bool) {
    tracing::warn!("BUILDING DEBUG PACKAGE!!!");
    tracing::info!("Checking cargo binutils install state");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root()).args(["objcopy", "--version"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy version fails");
    if !status.success() {
        tracing::info!("Please install cargo binutils with these commands:");
        tracing::info!("cargo install cargo-binutils");
        tracing::info!("rustup component add llvm-tools");
        exit(-1);
    }

    tracing::info!("Cargo clean...");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd
        .current_dir(project_root())
        .args(["clean", "--release", "--package", "firmware", "--package", "bootloader"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo clean fails");
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

fn build_bt_bootloader(verbose: bool) {
    tracing::info!("Building bootloader....");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd
        .current_dir(project_root().join("bootloader"))
        .args(["build", "--release"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null()).arg("--quiet");
    }
    let status = cmd.status().expect("Running Cargo failed");
    if !status.success() {
        tracing::error!("Bootloader build failed");
        exit(-1);
    }

    // Create bootloader binary first to show actual size
    tracing::info!("Creating bootloader binary file");
    let mut cargo_cmd = Command::new(cargo());
    let cmd = cargo_cmd
        .current_dir(project_root().join("bootloader"))
        .args(["objcopy", "--release"]);
    let mut cmd = cmd.args(["--", "-j", ".text", "-O", "binary", "../BtPackage/bootloader.bin"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Bootloader binary generation failed");
        exit(-1);
    }

    // Print bootloader binary size information
    print_bootloader_binary_size(&project_root().join("BtPackage/bootloader.bin"), "Bootloader Binary (actual size)");

    tracing::info!("Generating bootloader hex file...");
    let mut cargo_cmd = Command::new(cargo());
    let cmd = cargo_cmd
        .current_dir(project_root().join("bootloader"))
        .args(["objcopy", "--release"]);
    let mut cmd = cmd.args(["--", "-O", "ihex", "../BtPackage/bootloader.hex"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Bootloader hex generation failed");
        exit(-1);
    }
}

fn build_bt_bootloader_debug(verbose: bool) {
    tracing::info!("Building debug bootloader....");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd
        .current_dir(project_root().join("bootloader"))
        .args(["build", "--release"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null()).arg("--quiet");
    }
    let status = cmd.status().expect("Running Cargo failed");
    if !status.success() {
        tracing::error!("Bootloader build failed");
        exit(-1);
    }

    // Create debug bootloader binary first to show actual size
    tracing::info!("Creating debug bootloader binary file");
    let mut cargo_cmd = Command::new(cargo());
    let cmd = cargo_cmd
        .current_dir(project_root().join("bootloader"))
        .args(["objcopy", "--release"]);
    let mut cmd = cmd.args(["--", "-j", ".text", "-O", "binary", "../BtPackageDebug/bootloaderDebug.bin"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Debug bootloader binary generation failed");
        exit(-1);
    }

    // Print debug bootloader binary size information
    print_bootloader_binary_size(
        &project_root().join("BtPackageDebug/bootloaderDebug.bin"),
        "Debug Bootloader Binary (actual size)",
    );

    tracing::info!("Generating bootloader hex file...");
    let mut cargo_cmd = Command::new(cargo());
    let cmd = cargo_cmd
        .current_dir(project_root().join("bootloader"))
        .args(["objcopy", "--release"]);
    let mut cmd = cmd.args(["--", "-O", "ihex", "../BtPackageDebug/bootloaderDebug.hex"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Bootloader hex generation failed");
        exit(-1);
    }
}

fn build_bt_firmware(verbose: bool) {
    tracing::info!("Building application...");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("firmware")).args([
        "build",
        "--release",
        "-Z",
        "build-std=panic_abort",
        "-Z",
        "build-std-features=panic_immediate_abort",
    ]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null()).arg("--quiet");
    }
    let status = cmd.status().expect("Running Cargo failed");
    if !status.success() {
        tracing::error!("Firmware build failed");
        exit(-1);
    }

    // Create unpadded binary first to show actual size
    let mut cargo_cmd = Command::new(cargo());
    let cmd = cargo_cmd
        .current_dir(project_root().join("firmware"))
        .args(["objcopy", "--release"]);
    let mut cmd = cmd.args(["--", "-O", "binary", "../BtPackage/BT_application_unpadded.bin"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Firmware build failed");
        exit(-1);
    }

    // Created a full populated flash image to avoid the signed fw is different from the slice to check.
    // We will always get the full slice of flash where app is flashed ( BASE_APP_ADDR up to BASE_BOOTLOADER_ADDR )
    tracing::info!("Creating BT application bin file");
    // Print actual binary size information
    print_binary_size(
        &project_root().join("BtPackage/BT_application_unpadded.bin"),
        "Firmware Binary (actual size)",
    );

    let mut cargo_cmd = Command::new(cargo());
    let base_bootloader_addr = BASE_BOOTLOADER_ADDR.to_string();
    let cmd = cargo_cmd
        .current_dir(project_root().join("firmware"))
        .args(["objcopy", "--release"]);
    let mut cmd = cmd.args([
        "--",
        "--pad-to",
        base_bootloader_addr.as_str(), // no need to reserve space for trailer because we don't use cosign2's extended signatures
        "-O",
        "binary",
        "../BtPackage/BT_application.bin",
    ]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Firmware build failed");
        exit(-1);
    }
}

fn build_bt_debug_firmware(verbose: bool) {
    tracing::info!("Building debug application...");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("firmware")).args(["build", "--release"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null()).arg("--quiet");
    }
    let status = cmd.status().expect("Running Cargo failed");
    if !status.success() {
        panic!("Firmware build failed");
    }

    tracing::info!("Creating BT application hex file");
    let mut cargo_cmd = Command::new(cargo());
    let cmd = cargo_cmd
        .current_dir(project_root().join("firmware"))
        .args(["objcopy", "--release"]);
    let mut cmd = cmd.args(["--", "-O", "ihex", "../BtPackageDebug/BtappDebug.hex"]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
    if !status.success() {
        panic!("Firmware build failed");
    }
}

fn sign_bt_firmware(config_path: &str, developer: bool) {
    let cosign2_config_path = project_root().join(config_path);
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

    let version = MetadataCommand::new()
        .exec()
        .expect("Failed to get workspace metadata")
        .packages
        .iter()
        .find(|p| p.name == "firmware")
        .map(|p| p.version.to_string())
        .expect("Target crate not found in workspace");

    let header_size = SIGNATURE_HEADER_SIZE.to_string();
    let signed_once = std::fs::metadata("./BtPackage/BT_application_signed_once.bin").is_ok();
    let mut args = vec!["sign", "-c", config_path, "-i"];
    if signed_once {
        args.push("./BtPackage/BT_application_signed_once.bin");
    } else {
        args.push("./BtPackage/BT_application.bin");
    }
    if developer {
        args.push("--developer");
    }
    args.extend(["--header-size", header_size.as_str(), "--binary-version", &version, "-o"]);
    if signed_once || developer {
        args.push("./BtPackage/BT_application_signed.bin");
    } else {
        args.push("./BtPackage/BT_application_signed_once.bin");
    }

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

fn merge_files<P: AsRef<Path>>(inputs: Vec<MergeableFile<P>>, patches: Option<Vec<(u32, u8)>>, output: P) {
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

    let mut patches = patches.unwrap_or_default();
    let mut patch = patches.pop();

    // iterate through records and push them to output vector
    for (addr, mut value) in records.into_iter() {
        let upper = addr >> 16;

        // write extend linear address record if it has changed
        if upper != segment_upper_address {
            out_records.push(ihex::Record::ExtendedLinearAddress(upper as u16));
        }

        // apply patches
        if let Some((patch_addr, patch_value)) = patch {
            let end = addr + value.len() as u32;
            if (addr..end).contains(&patch_addr) {
                value[(patch_addr - addr) as usize] = patch_value;
                patch = patches.pop();
            }
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

fn patches_7_3_0(val: u8) -> Option<Vec<(u32, u8)>> {
    Some(vec![(0x0001_AE39, val), (0x0001_ADB9, val), (0x0000_3009, val), (0x0000_12F5, val)])
}

fn build_bt_package() {
    tracing::info!("Merging softdevice, bootloader and BT signed application in single hex");
    merge_files(
        vec![
            MergeableFile::IHex(project_root().join("misc/s113_nrf52_7.3.0_softdevice.hex")),
            MergeableFile::Binary(project_root().join("BtPackage/BT_application_signed.bin"), BASE_APP_ADDR),
            MergeableFile::IHex(project_root().join("BtPackage/bootloader.hex")),
        ],
        patches_7_3_0(0xB4),
        project_root().join("BtPackage/BTApp_Full_Image.hex"),
    );

    tracing::info!("Removing temporary files");
    let status = Command::new("rm")
        .current_dir(project_root().join("BtPackage"))
        .arg("-rf")
        .arg("bootloader.hex")
        .status()
        .expect("Running rm failed");
    if !status.success() {
        tracing::error!("Removing single hex files failed");
        exit(-1)
    }
}

fn build_bt_minimal_package() {
    tracing::info!("Merging softdevice and bootloader in single hex");
    merge_files(
        vec![
            MergeableFile::IHex(project_root().join("misc/s113_nrf52_7.3.0_softdevice.hex")),
            MergeableFile::IHex(project_root().join("BtPackage/bootloader.hex")),
        ],
        patches_7_3_0(0xB4),
        project_root().join("BtPackage/BT_Minimal_Image.hex"),
    );

    tracing::info!("Removing temporary files");
    let status = Command::new("rm")
        .current_dir(project_root().join("BtPackage"))
        .arg("-rf")
        .arg("bootloader.hex")
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
            MergeableFile::IHex(project_root().join("misc/s113_nrf52_7.3.0_softdevice.hex")),
            MergeableFile::IHex(project_root().join("BtPackageDebug/BtappDebug.hex")),
            MergeableFile::IHex(project_root().join("BtPackageDebug/bootloaderDebug.hex")),
        ],
        patches_7_3_0(0xB4),
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

fn patch_sd() {
    merge_files(
        vec![MergeableFile::IHex(project_root().join("misc/s113_nrf52_7.3.0_softdevice.hex"))],
        patches_7_3_0(0xB4),
        project_root().join("misc/s113_nrf52_7.3.0_softdevice_patched.hex"),
    );
}

fn print_binary_size(binary_path: &Path, description: &str) {
    if let Ok(metadata) = fs::metadata(binary_path) {
        let size_bytes = metadata.len();
        let size_kb = size_bytes as f64 / 1024.0;

        // Calculate flash usage percentage
        // Available flash space for application: from BASE_APP_ADDR to BASE_BOOTLOADER_ADDR, minus signature header
        let app_flash_size = (BASE_BOOTLOADER_ADDR - BASE_APP_ADDR - SIGNATURE_HEADER_SIZE) as u64;
        let usage_percentage = (size_bytes as f64 / app_flash_size as f64) * 100.0;

        println!("📊 {} Size:", description);
        println!("   Bytes: {} bytes", size_bytes);
        println!("   KiB: {:.2} KiB", size_kb);
        println!("   Flash Usage: {:.1}% of {} bytes available", usage_percentage, app_flash_size);
    } else {
        tracing::warn!("Could not read binary metadata for: {}", binary_path.display());
    }
}

fn print_bootloader_binary_size(binary_path: &Path, description: &str) {
    if let Ok(metadata) = fs::metadata(binary_path) {
        let size_bytes = metadata.len();
        let size_kb = size_bytes as f64 / 1024.0;

        // Calculate flash usage percentage for bootloader
        // Bootloader flash space: from BASE_BOOTLOADER_ADDR to end of flash (192K)
        let total_flash_size = 192 * 1024; // 192K in bytes
        let bootloader_flash_size = total_flash_size - BASE_BOOTLOADER_ADDR as u64;
        let usage_percentage = (size_bytes as f64 / bootloader_flash_size as f64) * 100.0;

        println!("📊 {} Size:", description);
        println!("   Bytes: {} bytes", size_bytes);
        println!("   KiB: {:.2} KiB", size_kb);
        println!(
            "   Flash Usage: {:.1}% of {} bytes available",
            usage_percentage, bootloader_flash_size
        );
    } else {
        tracing::warn!("Could not read binary metadata for: {}", binary_path.display());
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
            build_tools_check(args.verbose);
            build_bt_bootloader(args.verbose);
            build_bt_firmware(args.verbose);
            sign_bt_firmware("cosign2.toml", true);
            build_bt_package();
        }
        Commands::BuildMinimalImage => {
            build_tools_check(args.verbose);
            build_bt_bootloader(args.verbose);
            build_bt_minimal_package();
        }
        Commands::BuildUnsigned => {
            build_tools_check(args.verbose);
            build_bt_bootloader(args.verbose);
            build_bt_firmware(args.verbose);
        }
        Commands::SignFirmware { config_path } => {
            sign_bt_firmware(&config_path, false);
        }
        Commands::PackageFirmware => {
            build_bt_package();
        }
        Commands::BuildFwDebugImage => {
            build_tools_check_debug(args.verbose);
            build_bt_bootloader_debug(args.verbose);
            build_bt_debug_firmware(args.verbose);
            build_bt_package_debug();
        }
        Commands::PatchSd => {
            patch_sd();
        }
    }
}
