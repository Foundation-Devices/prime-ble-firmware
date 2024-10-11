use clap::{Args, Parser, Subcommand};
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command,Stdio};


#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct XtaskArgs {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
   
    /// Build a full flashable firmware image, combining the bootloader, the recovery and normal images.
    /// Run the following first (in this order):
    ///     - build-bootloader
    ///     - build --recovery
    ///     - build
    #[command(verbatim_doc_comment)]
    BuildFirmwareImage,
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR")).ancestors().nth(1).unwrap().to_path_buf()
}

pub fn cargo() -> String { env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()) }


fn main() {
    let args = XtaskArgs::parse();

    match args.command {
        Commands::BuildFirmwareImage  => {

            eprintln!("Building application...");
            let status = Command::new(cargo())
            .current_dir(project_root().join("firmware"))
            .arg("build")
            .arg("-r")
            .arg("-q")
            .status()
            .expect("Running Cargo failed");
            if !status.success() {
                panic!("Firmware build failed");
            }

            eprintln!("Creating BT application hex file");
            let status = Command::new(cargo())
            .current_dir(project_root().join("firmware"))
            .args(["objcopy", "--release", "--", "-O", "ihex", "btapp.hex"])
            .status()
            .expect("Running Cargo objcopy failed");
            if !status.success() {
                panic!("Firmware build failed");
            }

            //cargo objcopy --release -- -O binary app.bin

            eprintln!("Building bootloader....");
            let status = Command::new(cargo())
            .current_dir(project_root().join("bootloader"))
            .arg("build")
            .arg("-r")
            .arg("-q")
            .status()
            .expect("Running Cargo failed");
            if !status.success() {
                panic!("Bootloader build failed");
            }

            eprintln!("Generating bootloader hex file...");
            let status = Command::new(cargo())
            .current_dir(project_root().join("bootloader"))
            .args(["objcopy", "--release", "--", "-O", "ihex", "bootloader.hex"])
            .status()
            .expect("Running Cargo objcopy failed");
            if !status.success() {
                panic!("Bootloader build failed");
            }

            eprintln!("Merging softdevice bootloader and BT application in single hex");
            let status = Command::new("srec_cat")
            .current_dir(project_root().join("misc"))
            .args(["../firmware/btapp.hex","-Intel","../bootloader/bootloader.hex","-Intel","./s112_nrf52_7.2.0_softdevice.hex","-Intel","-o","full.hex","-Intel"])
            .status()
            .expect("Running Cargo failed");
            if !status.success() {
                panic!("Bootloader build failed");
            }




            
            

        }
    }
}
