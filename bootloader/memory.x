# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
# SPDX-License-Identifier: GPL-3.0-or-later

MEMORY
{
    /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* These values correspond to the NRF52805 with SoftDevices S112 7.2.0 */
  FLASH (rx) : ORIGIN = 0x27000, LENGTH = 36K
  RAM : ORIGIN = 0x20000008, LENGTH = 24K - 8
  uicr_bootloader_start_address (r) : ORIGIN = 0x10001014, LENGTH = 0x4
}

SECTIONS {
     .uicr_bootloader_start_address :  {
       KEEP(*(.uicr_bootloader_start_address))
       . = ALIGN(4);
     } > uicr_bootloader_start_address
};


