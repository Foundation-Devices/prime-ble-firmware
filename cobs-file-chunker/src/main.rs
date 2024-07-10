use host_protocol::{Bootloader, HostProtocolMessage};
use postcard::to_slice_cobs;
use std::io::{stdin, stdout, BufRead, Write};
use std::{thread, time::Duration};

// Size of the chunk of app data
const CHUNK_SIZE: usize = 256;
// Read application file bin as bytes
static APPLICATION: &[u8] = include_bytes!("../firmware.bin");

fn main() {
    let mut buf = [0u8; 512];

    // Open port
    let mut port = serialport::new("/dev/ttyUSB0", 460800)
        .timeout(Duration::from_millis(10))
        .open()
        .expect("error");

    {
        let mut user_input = String::new();
        let _input = stdin();
        print!("Press Enter to erase flash memory");
        stdout().flush().unwrap();
        let mut answer = stdin().lock();
        answer.read_line(&mut user_input).unwrap();
    }

    // println!("Data in chunk {:?}", block.get_block_data().unwrap());
    let cobs_tx = to_slice_cobs(
        &HostProtocolMessage::Bootloader(Bootloader::EraseFirmware),
        &mut buf,
    )
    .unwrap();
    port.write_all(cobs_tx).expect("Write failed!");
    thread::sleep(Duration::from_millis(50));

    {
        let mut user_input = String::new();
        let _input = stdin();
        print!("Press Enter to send file for update");
        stdout().flush().unwrap();
        let mut answer = stdin().lock();
        answer.read_line(&mut user_input).unwrap();
    }
    // Cycle until application binary is over
    for app_chunk in APPLICATION.chunks(CHUNK_SIZE).enumerate() {
        // Prepare cobs packet to send to bootloader
        let block = Bootloader::WriteFirmwareBlock {
            block_idx: app_chunk.0,
            block_data: app_chunk.1,
        };
        println!("Preparing chunk idx {}", app_chunk.0);
        // println!("Data in chunk {:?}", block.get_block_data().unwrap());
        let cobs_tx =
            to_slice_cobs(&HostProtocolMessage::Bootloader(block.clone()), &mut buf).unwrap();
        port.write_all(cobs_tx).expect("Write failed!");
        thread::sleep(Duration::from_millis(30));
    }
}
