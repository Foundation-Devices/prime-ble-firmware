<!-- SPDX-FileCopyrightText: 2024 Foundation Devices, Inc. <hello@foundation.xyz> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

### **Guide for programming nRF52805**

#### Tools required:

* __Hardware tools__:
  
  [J-Link](https://www.segger.com/products/debug-probes/j-link)
  [ST-Link on-board converted into J-Link](https://www.segger.com/products/debug-probes/j-link/models/other-j-links/st-link-on-board)

  To ensure that users without root privileges can use the debug probe, it is recommended to configure udev as described in [udev rules](https://probe.rs/docs/getting-started/probe-setup/#linux%3A-udev-rules).
  
* __Software tools__:

  [Probe-rs](https://probe.rs/)
 
  To install from linux execute the following snippet from terminal:
  
  ```Bash
  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh
  ```
  to verify if the tool is correctly installed connect to the pc the J-Link and from the terminal do a `probe-rs list` command and should get something like this:
    ```Bash
      The following debug probes were found:
      [0]: J-Link -- 1366:0105:000775635523 (J-Link)
    ```

  Now you can copy the `hex` file with the firmware image in any folder you like and then, with the board powered, from terminal:
    ```Bash
    probe-rs download <PATH_TO_HEX> --chip nrf52805_xxAA --verify --binary-format hex
    ```
    
  In case you have more than one probe connected to your pc, for example:
  ```Bash
    probe-rs list
    The following debug probes were found:
    [0]: STLink V2 -- 0483:3748:51C3BF720648C2885249524621C287 (ST-LINK)
    [1]: J-Link Ultra -- 1366:0101:000504501469 (J-Link)
  ```
  you have to specify which probe you want to use with the `--probe` parameter using the complete id from the list command, in the example above to select STLink:

  ```Bash
  probe-rs download <PATH_TO_HEX> --chip nrf52805_xxAA --verify --binary-format hex --probe 1366:0101:000504501469
  ```
  
  A progress bar will confirm that flash programming is running.
  
   ```Bash
   Erasing ✔ [00:00:02] [###################################################################################################] 100.00 KiB/100.00 KiB @ 40.74 KiB/s (eta 0s )
   Programming ✔ [00:00:01] [###################################################################################################] 100.00 KiB/100.00 KiB @     63.78 KiB/s (eta 0s )    Finished in 4.964s
   ```
  Just repeat download command to program another mcu.
