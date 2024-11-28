// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! This module handles jumping to and executing the main application firmware
//! after bootloader operations are complete.

use crate::consts::INT_VECTOR_TABLE_BASE;
use cortex_m::peripheral::NVIC;
use defmt::info;
use embassy_nrf::interrupt::Interrupt;
use nrf_softdevice_mbr as mbr;
#[cfg(all(feature = "boot-signed-fw", feature = "s112"))]
use nrf_softdevice_s112::sd_softdevice_vector_table_base_set;
#[cfg(all(feature = "boot-signed-fw", feature = "s113"))]
use nrf_softdevice_s113::sd_softdevice_vector_table_base_set;

/// Boots the application assuming softdevice is present.
///
/// This function performs the following steps:
/// 1. Sets up the SoftDevice based on firmware type (signed/unsigned)
/// 2. Disables active interrupts that could interfere with the jump
/// 3. Updates the vector table base address for signed firmware
/// 4. Retrieves the Main Stack Pointer (MSP) and Reset Vector (RV) from the vector table
/// 5. Executes assembly code to:
///    - Switch to MSP as the active stack pointer
///    - Initialize the stack pointer
///    - Set link register to prevent returns
///    - Jump to the application reset vector
///
/// # Safety
///
/// This modifies the stack pointer and reset vector and will run code placed in the active partition.
/// This function never returns as it jumps directly to the application code.
pub unsafe fn jump_to_app() -> ! {
    #[cfg(feature = "boot-unsigned-fw")]
    // Set SD base address in case fw is just above
    // This configures the Master Boot Record (MBR) to forward interrupts to the
    // SoftDevice at address 0x1000, which is required for unsigned firmware.
    let command = mbr::NRF_MBR_COMMANDS_SD_MBR_COMMAND_IRQ_FORWARD_ADDRESS_SET;
    #[cfg(feature = "boot-signed-fw")]
    // Set SD base address in case fw is in a specific location
    // This initializes the SoftDevice at address BASE_APP_ADDR, which is required
    // for signed firmware that is placed at a specific location in flash.
    let command = mbr::NRF_MBR_COMMANDS_SD_MBR_COMMAND_INIT_SD;

    let mut cmd = mbr::sd_mbr_command_t {
        command,
        params: mbr::sd_mbr_command_t__bindgen_ty_1 {
            irq_forward_address_set: mbr::sd_mbr_command_irq_forward_address_set_t {
                address: INT_VECTOR_TABLE_BASE,
            },
        },
    };
    let ret = mbr::sd_mbr_command(&mut cmd);
    info!("ret forward irq mbr result {}", ret);

    // Disable active interrupts
    NVIC::mask(Interrupt::UARTE0_UART0);
    NVIC::mask(Interrupt::RNG);

    // Probably this critical section is redundant, but keepin it for softdevice.
    critical_section::with(|_| {
        #[cfg(feature = "boot-signed-fw")]
        sd_softdevice_vector_table_base_set(INT_VECTOR_TABLE_BASE);

        let addr_header = INT_VECTOR_TABLE_BASE;
        let msp = *(addr_header as *const u32);
        let rv = *((addr_header + 4) as *const u32);

        info!("msp = {=u32:x}, rv = {=u32:x}", msp, rv);

        // These instructions perform the following operations:
        //
        // * Modify control register to use MSP as stack pointer (clear spsel bit)
        // * Synchronize instruction barrier
        // * Initialize stack pointer (0x1000)
        // * Set link register to not return (0xFF)
        // * Jump to softdevice reset vector
        core::arch::asm!(
            "mrs {tmp}, CONTROL",
            "bics {tmp}, {spsel}",
            "msr CONTROL, {tmp}",
            "isb",
            "msr MSP, {msp}",
            "mov lr, {new_lr}",
            "bx {rv}",
            // `out(reg) _` is not permitted in a `noreturn` asm! call,
            // so instead use `in(reg) 0` and don't restore it afterwards.
            tmp = in(reg) 0,
            spsel = in(reg) 2,
            new_lr = in(reg) 0xFFFFFFFFu32,
            msp = in(reg) msp,
            rv = in(reg) rv,
            options(noreturn),
        );
    })
}
