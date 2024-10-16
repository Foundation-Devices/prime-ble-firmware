### **Guide for programming Nrf52805**

#### Tools required:

* __Hardware tools__:
  
  [StLink V2 or V3](https://www.st.com/en/development-tools/st-link-v2.html#overview)

  The following versions of the ST-Link are supported:
    ```
      ST-Link V2, Firmware version 2.26 or higher
      ST-Link V3, Firmware version 3.2 or higher
    ```
  If you get an error message indicating that the firmware is outdated, please use the official ST tools to update the firmware.
  The update tool can be found on the [ST website](https://www.st.com/en/development-tools/stsw-link007.html).
  No additional drivers are required to use a ST-Link debug probe on Linux systems.
  
  To ensure that users without root privileges can use the debug probe, it is recommended to configure udev as described in [udev rules](https://probe.rs/docs/getting-started/probe-setup/#linux%3A-udev-rules).
  
* __Software tools__:

  [Probe-rs](https://probe.rs/)
 
  To install from linux execute the following snippet from terminal:
  
  ```Bash
  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh
  ```
  to verify if the tool is correctly installed connect to the pc the ST Link V2 and from the terminal do a `probe-rs list` command and should get something like this:
    ```Bash
      The following debug probes were found:
      [0]: STLink V2 -- 0483:3748:51C3BF720648C2885249524621C287 (ST-LINK)
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
  probe-rs download <PATH_TO_HEX> --chip nrf52805_xxAA --verify --binary-format hex --probe 0483:3748:51C3BF720648C2885249524621C287
  ```
  
  A progress bar will confirm that flash programming is running.
  
   ```Bash
   Erasing ✔ [00:00:02] [###################################################################################################] 100.00 KiB/100.00 KiB @ 40.74 KiB/s (eta 0s )
   Programming ✔ [00:00:01] [###################################################################################################] 100.00 KiB/100.00 KiB @     63.78 KiB/s (eta 0s )    Finished in 4.964s
   ```
  Just repeat download command to program another mcu.
