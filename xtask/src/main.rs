// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

use cargo_metadata::semver::Version;
use clap::{Parser, Subcommand};
use consts::{BASE_APP_ADDR_S112, BASE_APP_ADDR_S113, BASE_BOOTLOADER_ADDR, SIGNATURE_HEADER_SIZE};
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
    #[arg(short, long)]
    rev_d: bool,
    #[arg(short, long)]
    s113: bool,
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a full flashable firmware image with:
    /// SoftDevice
    /// Bootloader in release version (MPU UART pins - baud rate 460800)
    /// Memory protection (no probe access and bootloader and SD MBR area protected)
    #[command(verbatim_doc_comment)]
    BuildFwImage,

    /// Build a full package image with SD, bootloader and application without:
    /// Flash protection
    /// UART pins are redirected to the console at 115200 baud rate
    #[command(verbatim_doc_comment)]
    BuildFwDebugImage,

    /// Patch SoftDevice hex file to save some space at the end of it
    #[command(verbatim_doc_comment)]
    PatchSd,
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

fn build_bt_bootloader(verbose: bool, rev_d: bool, s113: bool) {
    tracing::info!("Building bootloader....");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("bootloader")).args([
        "build",
        "--release",
        "--features",
        match (rev_d, s113) {
            (true, true) => "hw-rev-d,s113",
            (false, true) => "s113",
            (true, false) => "hw-rev-d,s112",
            (false, false) => "s112",
        },
    ]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null()).arg("--quiet");
    }
    let status = cmd.status().expect("Running Cargo failed");
    if !status.success() {
        tracing::error!("Bootloader build failed");
        exit(-1);
    }

    tracing::info!("Generating bootloader hex file...");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("bootloader")).args([
        "objcopy",
        "--release",
        "--features",
        match (rev_d, s113) {
            (true, true) => "hw-rev-d,s113",
            (false, true) => "s113",
            (true, false) => "hw-rev-d,s112",
            (false, false) => "s112",
        },
        "--",
        "-O",
        "ihex",
        "../BtPackage/bootloader.hex",
    ]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Bootloader hex generation failed");
        exit(-1);
    }
}

fn build_bt_bootloader_debug(verbose: bool, rev_d: bool, s113: bool) {
    tracing::info!("Building debug bootloader....");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("bootloader")).args([
        "build",
        "--release",
        "--features",
        match (rev_d, s113) {
            (true, true) => "debug,hw-rev-d,s113",
            (false, true) => "debug,s113",
            (true, false) => "debug,hw-rev-d,s112",
            (false, false) => "debug,s112",
        },
    ]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null()).arg("--quiet");
    }
    let status = cmd.status().expect("Running Cargo failed");
    if !status.success() {
        tracing::error!("Bootloader build failed");
        exit(-1);
    }

    tracing::info!("Generating bootloader hex file...");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("bootloader")).args([
        "objcopy",
        "--release",
        "--features",
        match (rev_d, s113) {
            (true, true) => "debug,hw-rev-d,s113",
            (false, true) => "debug,s113",
            (true, false) => "debug,hw-rev-d,s112",
            (false, false) => "debug,s112",
        },
        "--",
        "-O",
        "ihex",
        "../BtPackageDebug/bootloaderDebug.hex",
    ]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
    if !status.success() {
        tracing::error!("Bootloader hex generation failed");
        exit(-1);
    }
}

fn build_bt_firmware(verbose: bool, rev_d: bool, s113: bool) {
    tracing::info!("Building application...");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("firmware")).args([
        "build",
        "--release",
        "--features",
        match (rev_d, s113) {
            (true, true) => "hw-rev-d,s113",
            (false, true) => "s113",
            (true, false) => "hw-rev-d,s112",
            (false, false) => "s112",
        },
    ]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null()).arg("--quiet");
    }
    let status = cmd.status().expect("Running Cargo failed");
    if !status.success() {
        tracing::error!("Firmware build failed");
        exit(-1);
    }

    tracing::info!("Creating BT application hex file");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("firmware")).args([
        "objcopy",
        "--release",
        "--features",
        match (rev_d, s113) {
            (true, true) => "hw-rev-d,s113",
            (false, true) => "s113",
            (true, false) => "hw-rev-d,s112",
            (false, false) => "s112",
        },
        "--",
        "-O",
        "ihex",
        "../BtPackage/BtApp.hex",
    ]);
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
    let mut cargo_cmd = Command::new(cargo());
    let base_bootloader_addr = BASE_BOOTLOADER_ADDR.to_string();
    let mut cmd = cargo_cmd.current_dir(project_root().join("firmware")).args([
        "objcopy",
        "--release",
        "--features",
        match (rev_d, s113) {
            (true, true) => "hw-rev-d,s113",
            (false, true) => "s113",
            (true, false) => "hw-rev-d,s112",
            (false, false) => "s112",
        },
        "--",
        "--pad-to",
        base_bootloader_addr.as_str(),
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

fn build_bt_debug_firmware(verbose: bool, rev_d: bool, s113: bool) {
    tracing::info!("Building debug application...");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("firmware")).args([
        "build",
        "--release",
        "--features",
        match (rev_d, s113) {
            (true, true) => "debug,hw-rev-d,s113",
            (false, true) => "debug,s113",
            (true, false) => "debug,hw-rev-d,s112",
            (false, false) => "debug,s112",
        },
    ]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null()).arg("--quiet");
    }
    let status = cmd.status().expect("Running Cargo failed");
    if !status.success() {
        panic!("Firmware build failed");
    }

    tracing::info!("Creating BT application hex file");
    let mut cargo_cmd = Command::new(cargo());
    let mut cmd = cargo_cmd.current_dir(project_root().join("firmware")).args([
        "objcopy",
        "--release",
        "--features",
        match (rev_d, s113) {
            (true, true) => "debug,hw-rev-d,s113",
            (false, true) => "debug,s113",
            (true, false) => "debug,hw-rev-d,s112",
            (false, false) => "debug,s112",
        },
        "--",
        "-O",
        "ihex",
        "../BtPackageDebug/BtappDebug.hex",
    ]);
    if !verbose {
        cmd = cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }
    let status = cmd.status().expect("Running Cargo objcopy failed");
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

    let header_size = SIGNATURE_HEADER_SIZE.to_string();
    let mut args = vec![
        "sign",
        "-i",
        "./BtPackage/BT_application.bin",
        "-c",
        "cosign2.toml",
        "--header-size",
        header_size.as_str(),
        "-o",
        "./BtPackage/BT_application_signed.bin",
    ];
    args.extend_from_slice(&["--firmware-version", &version]);

    tracing::info!("Signing binary Bt application with Cosign2...");

    // TODO: SFT-3595 sign again with second key

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

fn patches_7_2_0(val: u8, s113: bool) -> Option<Vec<(u32, u8)>> {
    if s113 {
        Some(vec![(0x0001_AF1D, val), (0x0001_AE9D, val), (0x0000_3009, val), (0x0000_12F5, val)])
    } else {
        Some(vec![(0x0001_8AA5, val), (0x0001_8A25, val), (0x0000_3009, val), (0x0000_1315, val)])
    }
}

fn patches_7_3_0(val: u8, s113: bool) -> Option<Vec<(u32, u8)>> {
    if s113 {
        Some(vec![(0x0001_AE39, val), (0x0001_ADB9, val), (0x0000_3009, val), (0x0000_12F5, val)])
    } else {
        Some(vec![(0x0001_89C1, val), (0x0001_8941, val), (0x0000_3009, val), (0x0000_1315, val)])
    }
}

fn build_bt_package(s113: bool) {
    tracing::info!("Merging softdevice, bootloader and BT signed application in single hex");
    merge_files(
        vec![
            MergeableFile::IHex(project_root().join(if s113 {
                "misc/s113_nrf52_7.3.0_softdevice.hex"
            } else {
                "misc/s112_nrf52_7.2.0_softdevice.hex"
            })),
            MergeableFile::Binary(
                project_root().join("BtPackage/BT_application_signed.bin"),
                if s113 { BASE_APP_ADDR_S113 } else { BASE_APP_ADDR_S112 },
            ),
            MergeableFile::IHex(project_root().join("BtPackage/bootloader.hex")),
        ],
        if s113 {
            patches_7_3_0(0xB4, s113)
        } else {
            patches_7_2_0(0x90, s113)
        },
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

fn build_bt_package_debug(s113: bool) {
    tracing::info!("Merging softdevice bootloader and BT signed application in single hex");
    merge_files(
        vec![
            MergeableFile::IHex(project_root().join(if s113 {
                "misc/s113_nrf52_7.3.0_softdevice.hex"
            } else {
                "misc/s112_nrf52_7.2.0_softdevice.hex"
            })),
            MergeableFile::IHex(project_root().join("BtPackageDebug/BtappDebug.hex")),
            MergeableFile::IHex(project_root().join("BtPackageDebug/bootloaderDebug.hex")),
        ],
        if s113 {
            patches_7_3_0(0xB4, s113)
        } else {
            patches_7_2_0(0x90, s113)
        },
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

fn patch_sd(s113: bool) {
    merge_files(
        vec![MergeableFile::IHex(project_root().join(if s113 {
            "misc/s113_nrf52_7.3.0_softdevice.hex"
        } else {
            "misc/s112_nrf52_7.2.0_softdevice.hex"
        }))],
        if s113 {
            patches_7_3_0(0xB4, s113)
        } else {
            patches_7_2_0(0x90, s113)
        },
        project_root().join(if s113 {
            "misc/s113_nrf52_7.3.0_softdevice_patched.hex"
        } else {
            "misc/s112_nrf52_7.2.0_softdevice_patched.hex"
        }),
    );
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
            build_bt_bootloader(args.verbose, args.rev_d, args.s113);
            build_bt_firmware(args.verbose, args.rev_d, args.s113);
            sign_bt_firmware();
            build_bt_package(args.s113);
        }
        Commands::BuildFwDebugImage => {
            build_tools_check_debug(args.verbose);
            build_bt_bootloader_debug(args.verbose, args.rev_d, args.s113);
            build_bt_debug_firmware(args.verbose, args.rev_d, args.s113);
            build_bt_package_debug(args.s113);
        }
        Commands::PatchSd => {
            patch_sd(args.s113);
        }
    }
}
