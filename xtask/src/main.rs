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
            let status = Command::new(cargo())
            .current_dir(project_root())
            .status()
            .expect("Running Cargo failed");
            if !status.success() {
                panic!("Local build failed");
            }

            let ls = Command::new(cargo())
            .current_dir(project_root().join("firmware"))
            .arg("build")
            .arg("-r")
            .status()
            .expect("sdsd");


            
            

        }
    }
}
