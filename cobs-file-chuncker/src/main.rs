use std::{thread, time::Duration};
use serialport::available_ports;
use postcard::to_slice_cobs;
use host_protocol::{HostProtocolMessage, Bootloader};


// Size of the chunk of app data
const CHUNK_SIZE : usize = 256;
// Read application file bin as bytes
static APPLICATION: &[u8] = include_bytes!("../firmware.bin");



fn main() {
    let mut buf = [0u8;512];

    // Open port
    let mut port = serialport::new("/dev/ttyUSB0", 115200)
    .timeout(Duration::from_millis(10))
    .open().expect("error");

    // let mut stdout = stdout();
    // stdout.write(b"Press Enter to start...").unwrap();
    // stdout.flush().unwrap();
    // stdin().read(&mut [0]).unwrap();

    // Cycle until application binary is over
    for app_chunk in APPLICATION.chunks(CHUNK_SIZE).enumerate(){
        // Prepare cobs packet to send to bootloader
        let block = Bootloader::WriteFirmwareBlock { block_idx: app_chunk.0 , block_data: app_chunk.1 };
        println!("Preparing chunk idx {}", app_chunk.0);
        // println!("Data in chunk {:?}", block.get_block_data().unwrap());
        let cobs_tx = to_slice_cobs(&HostProtocolMessage::Bootloader(block.clone()), &mut buf).unwrap();
        port.write_all(cobs_tx).expect("Write failed!");
        thread::sleep(Duration::from_millis(50));
    }

    // let output = "This is a test. This is only a test.".as_bytes();
    // port.write(output).expect("Write failed!");

}
