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

    #[cfg(all(not(feature = "debug"), feature = "s112"))]
    let memory_x_content = format!(
        r##"
        BASE_BOOTLOADER_ADDR = {:#X};
        BASE_APP_ADDR = {:#X};
        SIGNATURE_HEADER_SIZE = {:#X};

        MEMORY
        {{
            /* NOTE 1 K = 1 KiBi = 1024 bytes */
            /* The SoftDevices S112 7.2.0 minimal RAM requirement is 3.7K (0xEB8) */
            /* and use a maximum of 1.75K (0x700) for call stack. */
            /* We choose to reserve 9968 bytes (0x26F0) at the begining of RAM */
            FLASH (rx) : ORIGIN = 0x00000000 + BASE_APP_ADDR + SIGNATURE_HEADER_SIZE, LENGTH = BASE_BOOTLOADER_ADDR - BASE_APP_ADDR - SIGNATURE_HEADER_SIZE
            RAM : ORIGIN = 0x20000000 + 9968, LENGTH = 24K - 9968
        }}
        "##,
        BASE_BOOTLOADER_ADDR, BASE_APP_ADDR, SIGNATURE_HEADER_SIZE
    );
    #[cfg(all(not(feature = "debug"), feature = "s113"))]
    let memory_x_content = format!(
        r##"
        BASE_BOOTLOADER_ADDR = {:#X};
        BASE_APP_ADDR = {:#X};
        SIGNATURE_HEADER_SIZE = {:#X};

        MEMORY
        {{
            /* NOTE 1 K = 1 KiBi = 1024 bytes */
            /* The SoftDevices S113 7.3.0 minimal RAM requirement is 4.4K (0x1198) */
            /* and use a maximum of 1.75K (0x700) for call stack. */
            /* We choose to reserve 10648 bytes (0x2998) at the begining of RAM */
            FLASH (rx) : ORIGIN = 0x00000000 + BASE_APP_ADDR + SIGNATURE_HEADER_SIZE, LENGTH = BASE_BOOTLOADER_ADDR - BASE_APP_ADDR - SIGNATURE_HEADER_SIZE
            RAM : ORIGIN = 0x20000000 + 10648, LENGTH = 24K - 10648
        }}
        "##,
        BASE_BOOTLOADER_ADDR, BASE_APP_ADDR, SIGNATURE_HEADER_SIZE
    );
    #[cfg(all(feature = "debug", feature = "s112"))]
    let memory_x_content = format!(
        r##"
        BASE_BOOTLOADER_ADDR = {:#X};
        BASE_APP_ADDR = {:#X};

        MEMORY
        {{
            /* NOTE 1 K = 1 KiBi = 1024 bytes */
            /* The SoftDevices S112 7.2.0 minimal RAM requirement is 3.7K (0xEB8) */
            /* and use a maximum of 1.75K (0x700) for call stack. */
            /* We choose to reserve 9968 bytes (0x26F0) at the begining of RAM */
            FLASH (rx) : ORIGIN = 0x00000000 + BASE_APP_ADDR, LENGTH = BASE_BOOTLOADER_ADDR - BASE_APP_ADDR
            RAM : ORIGIN = 0x20000000 + 9968, LENGTH = 24K - 9968
        }}
        "##,
        BASE_BOOTLOADER_ADDR, BASE_APP_ADDR
    );
    #[cfg(all(feature = "debug", feature = "s113"))]
    let memory_x_content = format!(
        r##"
        BASE_BOOTLOADER_ADDR = {:#X};
        BASE_APP_ADDR = {:#X};

        MEMORY
        {{
            /* NOTE 1 K = 1 KiBi = 1024 bytes */
            /* The SoftDevices S113 7.3.0 minimal RAM requirement is 4.4K (0x1198) */
            /* and use a maximum of 1.75K (0x700) for call stack. */
            /* We choose to reserve 10648 bytes (0x2998) at the begining of RAM */
            FLASH (rx) : ORIGIN = 0x00000000 + BASE_APP_ADDR, LENGTH = BASE_BOOTLOADER_ADDR - BASE_APP_ADDR
            RAM : ORIGIN = 0x20000000 + 10648, LENGTH = 24K - 10648
        }}
        "##,
        BASE_BOOTLOADER_ADDR, BASE_APP_ADDR
    );
    File::create(out.join("./memory.x"))
        .unwrap()
        .write_all(memory_x_content.as_bytes())
        .unwrap();

    println!("cargo:rustc-link-search={}", out.display());

    // By default, Cargo will re-run a build script whenever
    // any file in the project changes. By specifying `memory.x`
    // here, we ensure the build script is only re-run when
    // `memory.x` is changed.
    #[cfg(all(not(feature = "debug"), feature = "s112"))]
    println!("cargo:rerun-if-changed=./memory_s112_signed.x");
    #[cfg(all(not(feature = "debug"), feature = "s113"))]
    println!("cargo:rerun-if-changed=./memory_s113_signed.x");
    #[cfg(all(feature = "debug", feature = "s112"))]
    println!("cargo:rerun-if-changed=./memory_s112_unsigned.x");
    #[cfg(all(feature = "debug", feature = "s113"))]
    println!("cargo:rerun-if-changed=./memory_s113_unsigned.x");

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
