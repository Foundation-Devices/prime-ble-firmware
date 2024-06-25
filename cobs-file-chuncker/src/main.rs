use std::time::Duration;
use serialport::available_ports;
use postcard::to_slice_cobs;
use host_protocol::{HostProtocolMessage, Bootloader};


// Size of the chunk of app data
const CHUNK_SIZE : usize = 256;
// Read application file bin as bytes
static APPLICATION: &[u8] = include_bytes!("../firmware");



fn main() {
    let mut buf = [0u8;512];
    let ports = serialport::available_ports().expect("No ports found!");
    for p in ports {
        println!("{}", p.port_name);
    }

    // Open port
    if let Ok(mut port) = serialport::new("/dev/ttyUSB0", 115_200)
    .timeout(Duration::from_millis(10))
    .open(){
        println!("COM opened!")
    }

    // Cycle until application binary is over
    for app_chunk in APPLICATION.chunks(CHUNK_SIZE).enumerate(){
        // Prepare cobs packet to send to bootloader
        let block = Bootloader::WriteFirmwareBlock { block_idx: app_chunk.0 , block_data: app_chunk.1 };
        println!("Preparing chunk idx {}", block.get_block_idx().unwrap());
        println!("Data in chunk {:?}", block.get_block_data().unwrap());
        let _cobs_tx = to_slice_cobs(&HostProtocolMessage::Bootloader(block.clone()), &mut buf).unwrap();
       
    }

    // let output = "This is a test. This is only a test.".as_bytes();
    // port.write(output).expect("Write failed!");

}
