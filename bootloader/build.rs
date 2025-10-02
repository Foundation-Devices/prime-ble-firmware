// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
// SPDX-License-Identifier: GPL-3.0-or-later

//! This build script copies the `memory.x` file from the crate root into
//! a directory where the linker can always find it at build time.
//! For many projects this is optional, as the linker always searches the
//! project root directory -- wherever `Cargo.toml` is. However, if you
//! are using a workspace or have a more complicated build setup, this
//! build script becomes required. Additionally, by requesting that
//! Cargo re-run the build script whenever `memory.x` is changed,
//! updating `memory.x` ensures a rebuild of the application with the
//! new memory settings.

use consts_global::BASE_BOOTLOADER_ADDR;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let memory_x_content = format!(
        r##"
        BASE_BOOTLOADER_ADDR = {:#X};

        MEMORY
        {{
            /* NOTE 1 K = 1 KiBi = 1024 bytes */
            /* The bootloader flash partition is the last 36K of flash */
            /* No need to reserve RAM for SoftDevice as it is not executed at all in bootloader */
            FLASH (rx) : ORIGIN = 0x00000000 + BASE_BOOTLOADER_ADDR, LENGTH = 192K - BASE_BOOTLOADER_ADDR
            RAM : ORIGIN = 0x20000008, LENGTH = 24K - 8
            mbr_uicr_bootloader_addr (r) : ORIGIN = 0x10001014, LENGTH = 0x4
            uicr_approtect (r) : ORIGIN = 0x10001208, LENGTH = 0x4
        }}

        SECTIONS {{
            .uicr_approtect :  {{
                KEEP(*(.uicr_approtect))
                . = ALIGN(4);
            }} > uicr_approtect

            .mbr_uicr_bootloader_addr :  {{
                KEEP(*(.mbr_uicr_bootloader_addr))
                . = ALIGN(4);
            }} > mbr_uicr_bootloader_addr
        }};
        "##,
        BASE_BOOTLOADER_ADDR
    );
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(memory_x_content.as_bytes())
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    println!("cargo:rerun-if-changed=./memory.x");

    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
