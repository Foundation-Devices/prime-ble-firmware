// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later
use defmt::{info,trace};
use crate::BASE_ADDRESS_APP;

/// Boots the application assuming softdevice is present.
///
/// # Safety
///
/// This modifies the stack pointer and reset vector and will run code placed in the active partition.
pub unsafe fn jump_to_app() -> ! {
    use nrf_softdevice_mbr as mbr;
    let app_addr = BASE_ADDRESS_APP + 0x0800;
    let mut cmd = mbr::sd_mbr_command_t {
        command: mbr::NRF_MBR_COMMANDS_SD_MBR_COMMAND_VECTOR_TABLE_BASE_SET,
        params: mbr::sd_mbr_command_t__bindgen_ty_1 {
                base_set: mbr::sd_mbr_command_vector_table_base_set_t {
                address: app_addr,
            },
        },
    };
    let ret = mbr::sd_mbr_command(&mut cmd);
    info!("ret val base set {}", ret);

    // Address of softdevice which we'll forward interrupts to
    let addr = 0x1000;
    let mut cmd = mbr::sd_mbr_command_t {
        command: mbr::NRF_MBR_COMMANDS_SD_MBR_COMMAND_IRQ_FORWARD_ADDRESS_SET,
        params: mbr::sd_mbr_command_t__bindgen_ty_1 {
            irq_forward_address_set: mbr::sd_mbr_command_irq_forward_address_set_t {
                address: addr,
            },
        },
    };
    let ret = mbr::sd_mbr_command(&mut cmd);
    info!("ret val irq fw{}", ret);

    let msp = *(app_addr as *const u32);
    let rv = *((app_addr + 4) as *const u32);

    trace!("msp = {=u32:x}, rv = {=u32:x}", msp, rv);

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
}
