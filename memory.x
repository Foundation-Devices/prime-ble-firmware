MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* These values correspond to the NRF52805 with SoftDevices S112 7.2.0 */
  FLASH (rx) : ORIGIN = 0x19000, LENGTH = 192K - 100K
  RAM : ORIGIN = 0x20000000 + 9400, LENGTH = 24K - 9400
}
