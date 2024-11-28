# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
# SPDX-License-Identifier: GPL-3.0-or-later

MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* The bootloader flash partition is the last 35.25K of flash (start at 0x27200) */
  /* The SoftDevices S112 7.2.0 flash partition is the first 100K of flash (end at 0x19000) */
  /* The SoftDevices S112 7.2.0 minimal RAM requirement is 3.7K (0xEB8) */
  /* and use a maximum of 1.75K (0x700) for call stack. */
  /* We choose to reserve 9968 bytes (0x26F8) at the begining of RAM */
  FLASH (rx) : ORIGIN = 0x00000000 + 100K, LENGTH = 0x27200 - 100K
  RAM : ORIGIN = 0x20000000 + 9968, LENGTH = 24K - 9968
}
