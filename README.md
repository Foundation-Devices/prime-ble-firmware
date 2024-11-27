## `prime-ble-firmware`

### Passport Prime BLE controller firmware.

This workspace contains the following crates:

- `bootloader`: Secure bootloader that handles firmware updates and verification
- `firmware`: Main BLE application firmware that implements the Bluetooth protocol
- `host-protocol`: Shared protocol definitions for MPU-BLE communication

The firmware and bootloader communicate with the main MCU via UART using the COBS protocol for reliable serial data transfer. The host protocol defines the message types and structures used for this communication.


### Prerequisites

   ```bash
   # Install xtask for custom build scripts
   cargo install cargo-xtask
   ```
   ```bash
   # Install just for running the Justfile
   cargo install just
   ```
   ```bash
   # Install binutils for working with binary files
   cargo install cargo-binutils
   ```
   ```bash
   # Install LLVM toolchain
   apt install llvm libclang-dev
   ```
   ```bash
   # Add LLVM tools component for binary inspection
   rustup component add llvm-tools
   ```
   ```bash
   # Add ARM Cortex-M4 compilation target
   rustup target add thumbv7em-none-eabi
   ```

### Installation and Running with probe-rs

Follow the [probe-rs installation guide](https://probe.rs/docs/getting-started/installation/) to get started.

1. Install the probe-rs tools:
   ```bash
   cargo install probe-rs-tools
   ```

2. Verify your ST-Link probe is detected:
   ```bash
   probe-rs list
   ```

3. Build firmware packages using the provided just commands:

   **Just command list**
   ```bash
   just -l
   ```
   **Build Production Package**
   ```bash
   just build
   ```
   This creates a `BtPackage` folder containing:
   - `BTApp_Full_Image.hex` - Complete production image in Intel HEX format
   - `BT_application.bin` - Raw Bluetooth application binary
   - `BT_application_signed.bin` - Signed application with cosign2 header for updates
  
   **Build Debug Package** 
   ```bash
   just build-debug
   ```
   This creates a `BtPackage` folder containing:
   - `BTApp_Full_Image_debug.hex` - Debug image with console UART and no flash protection

   **Flash SoftDevice and run Bluetooth test app with UART MPU** 
   ```bash
   just bluetooth-app
   ```



### Fixing `JtagNoDeviceConnected` error

Some nRF52 chips are coming locked from the fab and need an unlocking procedure to be programmed.
The unlocking requires a J-Link probe and cannot be done with ST-Link probe.

- Install `nrf-recover` tool
  ```bash
  cargo install nrf-recover
  ```

- Connect the J-Link `SWD` wires (as well as `VCC` and `GND`) to the nRF52 programming port.
  The easiest way to do it is to use tag-connect 20-to-10 ribbon cable converter board.
  If this converter isn't available, you can connect the wires manually.

- Run the `nrf-recover` tool while selecting the J-Link probe:
  ```bash
  nrf-recover --probe-index 0 -y
  ```

- The result should look like this:
  ```
  Starting mass erase...
  Mass erase completed, chip unlocked
  ```

- Power cycle the board and try to program the `SoftDevice` again.




### Notes about `Access port protection` use and chip revisions

As reported by [Informational Notice](./misc/in_153_v1.0.1.pdf) there was a change in the way 'Access port protection' is used in rev. B of the chip due to a possible attack.

You can read about in [nrf52805 datasheet](./misc/nRF52805_PS_v1.4.pdf) at paragraph _4.8.2_ and you can read about the different methods of lock/unlock based on chip revision.

Be careful that reading, with a probe-rs read command, info bytes at paragraph _4.4.1.9 INFO.VARIANT_ in datasheet we get from prime board rev. Y2 these values:

 ```
  00052805 41414130 00002004 
```

that are indicating that chip is still of rev. A not variant B with the updated protection command.

As explained in Informational Notice chip of rev. A are still produced ( probably for legacy ) and in case we want to switch to patched protection sequence a different part number must be ordered.

