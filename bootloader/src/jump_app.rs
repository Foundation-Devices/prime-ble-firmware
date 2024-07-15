// SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
// SPDX-License-Identifier: GPL-3.0-or-later
use crate::consts::BASE_ADDRESS_APP;
use cortex_m::peripheral::NVIC;
use defmt::info;
use embassy_nrf::interrupt::Interrupt;
use nrf_softdevice_s112::sd_softdevice_vector_table_base_set;

/// Boots the application assuming softdevice is present.
///
/// # Safety
///
/// This modifies the stack pointer and reset vector and will run code placed in the active partition.
pub unsafe fn jump_to_app() -> ! {
    use nrf_softdevice_mbr as mbr;

    let addr = 0x1000;
    let mut cmd = mbr::sd_mbr_command_t {
        command: mbr::NRF_MBR_COMMANDS_SD_MBR_COMMAND_INIT_SD,
        params: mbr::sd_mbr_command_t__bindgen_ty_1 {
            irq_forward_address_set: mbr::sd_mbr_command_irq_forward_address_set_t { address: 0x19000 },
        },
    };
    let ret = mbr::sd_mbr_command(&mut cmd);

    info!("ret SD init result {}", ret);

    // Disable active interrupts
    NVIC::mask(Interrupt::UARTE0_UART0);
    NVIC::mask(Interrupt::RNG);

    // Probably this critical section is redundant, but keepin it for softdevice.
    critical_section::with(|_| {
        let ret = sd_softdevice_vector_table_base_set(BASE_ADDRESS_APP);
        info!("ret val base set {}", ret);

        let mut cmd = mbr::sd_mbr_command_t {
            command: mbr::NRF_MBR_COMMANDS_SD_MBR_COMMAND_IRQ_FORWARD_ADDRESS_SET,
            params: mbr::sd_mbr_command_t__bindgen_ty_1 {
                irq_forward_address_set: mbr::sd_mbr_command_irq_forward_address_set_t { address: addr },
            },
        };
        let ret = mbr::sd_mbr_command(&mut cmd);

        info!("ret forward irq mbr result {}", ret);

        let addr_header = BASE_ADDRESS_APP;
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
