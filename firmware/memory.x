# SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundationdevices.com>
# SPDX-License-Identifier: GPL-3.0-or-later

MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* These values correspond to the NRF52805 with SoftDevices S112 7.2.0 */
  /* Need to add header size here to origin (0x800) to make SD boot correctly */
  /* NOTE if you want to use unsigned firmware use flash ORIGIN 0x19000       */
  FLASH (rx) : ORIGIN = 0x19000, LENGTH = 52K
  RAM : ORIGIN = 0x20000000 + 9968, LENGTH = 24K - 9968
}

