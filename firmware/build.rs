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

#[cfg(not(feature = "debug"))]
use consts::SIGNATURE_HEADER_SIZE;
use consts::{BASE_APP_ADDR, BASE_BOOTLOADER_ADDR};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    // Put `memory.x` in our output directory and ensure it's
    // on the linker search path.
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());

    #[cfg(feature = "debug")]
    let signature_header_size = 0;
    #[cfg(not(feature = "debug"))]
    let signature_header_size = SIGNATURE_HEADER_SIZE;
    /* The SoftDevices S113 7.3.0 minimal RAM requirement is 4.4K (0x1198) */
    /* and use a maximum of 1.75K (0x700) for call stack. */
    /* We choose to reserve 10648 bytes (0x2998) at the begining of RAM */
    let soft_device_ram_reserved = 10648;

    let memory_x_content = format!(
        r##"
        BASE_BOOTLOADER_ADDR = {:#X};
        BASE_APP_ADDR = {:#X};
        SIGNATURE_HEADER_SIZE = {};

        MEMORY
        {{
            /* NOTE 1 K = 1 KiBi = 1024 bytes */
            FLASH (rx) : ORIGIN = 0x00000000 + BASE_APP_ADDR + SIGNATURE_HEADER_SIZE, LENGTH = BASE_BOOTLOADER_ADDR - BASE_APP_ADDR - SIGNATURE_HEADER_SIZE
            RAM : ORIGIN = 0x20000000 + {}, LENGTH = 24K - {}
        }}
        "##,
        BASE_BOOTLOADER_ADDR, BASE_APP_ADDR, signature_header_size, soft_device_ram_reserved, soft_device_ram_reserved
    );
    File::create(out.join("./memory.x"))
        .unwrap()
        .write_all(memory_x_content.as_bytes())
        .unwrap();

    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
