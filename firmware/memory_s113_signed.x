# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
# SPDX-License-Identifier: GPL-3.0-or-later

MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* The bootloader flash partition is the last 35.25K of flash (start at 0x27200) */
  /* The signature take 256 bytes (0x100) of flash at the begining of te application partition */
  /* The SoftDevices S113 7.3.0 flash partition is the first 112K of flash (end at 0x1B400) */
  /* The SoftDevices S113 7.3.0 minimal RAM requirement is 4.4K (0x1198) */
  /* and use a maximum of 1.75K (0x700) for call stack. */
  /* We choose to reserve 10648 bytes (0x2998) at the begining of RAM */
  FLASH (rx) : ORIGIN = 0x00000000 + 0x1B400 + 256, LENGTH = 0x27200 - 0x1B400 - 256
  RAM : ORIGIN = 0x20000000 + 10648, LENGTH = 24K - 10648
}
