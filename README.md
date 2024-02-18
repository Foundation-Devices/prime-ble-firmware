## `prime-ble-firmware`

Passport Prime BLE controller firmware.

### Installation and running

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
   cargo run --release -- --probe <PROBE>
   ```
