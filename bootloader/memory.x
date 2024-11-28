# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
# SPDX-License-Identifier: GPL-3.0-or-later

MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* The bootloader flash partition is the last 35.9K of flash (start at 0x27200) */
  /* No need to reserve RAM for SoftDevice as it is not executed at all in bootloader */
  FLASH (rx) : ORIGIN = 0x00000000 + 0x27000, LENGTH = 192K - 0x27000
  RAM : ORIGIN = 0x20000008, LENGTH = 24K - 8
  uicr_bootloader_start_address (r) : ORIGIN = 0x10001014, LENGTH = 0x4
  uicr_appprotection (r) : ORIGIN = 0x10001208, LENGTH = 0x4
}

SECTIONS {
    .uicr_appprotection :  {
       KEEP(*(.uicr_appprotection))
       . = ALIGN(4);
    } > uicr_appprotection

    .uicr_bootloader_start_address :  {
       KEEP(*(.uicr_bootloader_start_address))
       . = ALIGN(4);
     } > uicr_bootloader_start_address
};


