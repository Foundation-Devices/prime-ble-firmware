# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz>
# SPDX-License-Identifier: GPL-3.0-or-later

MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* The bootloader flash partition is the last 36K of flash (start at 0x27000) */
  /* No need to reserve RAM for SoftDevice as it is not executed at all in bootloader */
  FLASH (rx) : ORIGIN = 0x00000000 + 0x27000, LENGTH = 192K - 0x27000
  RAM : ORIGIN = 0x20000008, LENGTH = 24K - 8
  mbr_uicr_bootloader_addr (r) : ORIGIN = 0x10001014, LENGTH = 0x4
  uicr_approtect (r) : ORIGIN = 0x10001208, LENGTH = 0x4
}

SECTIONS {
    .uicr_approtect :  {
       KEEP(*(.uicr_approtect))
       . = ALIGN(4);
    } > uicr_approtect

    .mbr_uicr_bootloader_addr :  {
       KEEP(*(.mbr_uicr_bootloader_addr))
       . = ALIGN(4);
     } > mbr_uicr_bootloader_addr
};


