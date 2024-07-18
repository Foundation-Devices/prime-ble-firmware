## `prime-ble-firmware`

### Passport Prime BLE controller firmware.

This workspace is composed of:

- Bootloader
- Application firmware
- Host protocol

Firmware and bootloader communicates via UART with the main MCU 

Leaving here both probe-rs-cli solution and probe-rs ( which is the new one), because on my machine seems having some issue with probe-rs while with probe-rs-cli works nice. Maybe it's just a problem of my setup

### Installation and running with older probe-rs-cli

1. Install `probe-rs-cli`:
   ```bash
   cargo install probe-rs-cli
   ```
   
2. List connected probes with `probe-rs-cli` and find the ST-Link in there:
   ```bash
   probe-rs-cli list
   ```

3. Flash the SoftDevice S112:
   ```bash
    probe-rs-cli download misc/s112_nrf52_7.2.0_softdevice.hex --chip nrf52805_xxAA --format hex --probe <PROBE>
   ```
   
4. Flash and run the firmware:
   ```bash
   cargo run --release --bin firmware -- --probe <PROBE>
   ```


### Installation and running with probe-rs

1. Install `probe-rs`:
   ```bash
   cargo install probe-rs-tools
   ```
   
2. List connected probes with `probe-rs` and find the ST-Link in there:
   ```bash
   probe-rs list
   ```

3. Flash the SoftDevice S112:
   ```bash
    probe-rs download misc/s112_nrf52_7.2.0_softdevice.hex --chip nrf52805_xxAA --binary-format hex --probe <PROBE>
   ```
   
4. Modify in .cargo folder of firmware:
   ```bash
   runner = "probe-rs run --chip nrf52805_xxAA"
   ```
   
6. Flash and run the firmware:
   ```bash
   cargo run --release --bin firmware -- --probe <PROBE>
   ```
