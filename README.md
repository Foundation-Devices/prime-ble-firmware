## `prime-ble-firmware`

### Passport Prime BLE controller firmware.

This workspace is composed of:

- Bootloader
- Application firmware
- Host protocol

Firmware and bootloader communicates via UART with the main MCU 

Leaving here both probe-rs-cli solution and probe-rs ( which is the new one), because on my machine seems having some issue with probe-rs while with probe-rs-cli works nice. Maybe it's just a problem of my setup

### Installation and running with probe-rs ( instructions [here](https://probe.rs/docs/getting-started/installation/) )

1. Install `probe-rs`:
   ```bash
   cargo install probe-rs-tools
   ```
   
2. List connected probes with `probe-rs` and check the ST-Link is connected:
   ```bash
   probe-rs list
   ```

3. There are two Xtasks available to create:
   1. Complete package for production in Intel Hex format with signed Bt Application, Softdevice and Bootloader - binary application file for update mode
      ```bash
      cargo xtask build-fw-image
      ```
   At the end of the process a *BtPackage* folder will be created in project root folder with 3 files inside
      * _BTApp_Full_Image.hex_- Full production image Intel hex format
      * _BT_application.bin_- Bluetooth application without cosign2 header
      * _BT_application_signed.bin_- Bluetooth application cosign2 header for fw update
   2. Complete package for debug without Cosign Header, console uart for debug and no flash protection.
      ```bash
      cargo xtask build-fw-debug-image
      ```
   At the end of the process a *BtPackage* folder will be created in project root folder with 1 file inside
      * BTApp_Full_Image_debug.hex_- Full debug image Intel hex format ( No signed, uart console, no flash protection )
   
4. Modify in .cargo folder of firmware:
   ```bash
   runner = "probe-rs run --chip nrf52805_xxAA"
   ```
   
5. Flash and run the firmware:
   ```bash
   cargo run --release --bin firmware -- --probe <PROBE>
   ```
