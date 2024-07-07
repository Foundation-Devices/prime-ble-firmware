use host_protocol::{Bootloader, HostProtocolMessage};
use postcard::accumulator::{CobsAccumulator, FeedResult};
use postcard::to_slice_cobs;
use std::io::{stdin, stdout, BufRead, Write};
use std::{thread, time::Duration};

// Size of the chunk of app data
const CHUNK_SIZE: usize = 256;
// Read application file bin as bytes
static APPLICATION: &[u8] = include_bytes!("../firmware_signed.bin");

fn main() {
    let mut buf = [0u8; 512];

    // Open port
    let mut port = serialport::new("/dev/ttyUSB0", 115200)
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
        // Wait ack
    //     // Loop for bootloader commands
    //     // This loop will be a while loop with gpio state as condition to exit...
  
    //     // Raw buffer - 32 bytes for the accumulator of cobs
    //     let mut raw_buf = Vec::new();
    //     // Create a cobs accumulator for data incoming
    //     let mut cobs_buf: CobsAccumulator<512> = CobsAccumulator::new();
    //     // Getting chars from Uart in a while loop
    //     while let Ok(n) = port.read_to_end(&mut raw_buf) {
    //         // Finished reading input
    //         if n == 0 {
    //             break;
    //         }
    //         println!("Data incoming {}", n);

    //         let buf = &raw_buf[..n];
    //         let mut window: &[u8] = buf;

    //         'cobs: while !window.is_empty() {
    //             window = match cobs_buf.feed_ref::<HostProtocolMessage>(window) {
    //                 FeedResult::Consumed => {
    //                     println!("consumed");
    //                     break 'cobs;
    //                 }
    //                 FeedResult::OverFull(new_wind) => {
    //                     println!("overfull");
    //                     new_wind
    //                 }
    //                 FeedResult::DeserError(new_wind) => {
    //                     println!("DeserError");
    //                     new_wind
    //                 }
    //                 FeedResult::Success { data, remaining } => {
    //                     println!("Remaining {} bytes", remaining.len());

    //                     match data {
    //                         HostProtocolMessage::Bluetooth(_) => (), // Message for application
    //                         HostProtocolMessage::Bootloader(Bootloader::AckWithIdxCrc { block_idx : _, crc:  _}) =>  { break },
    //                         _ => (),
    //                     };
    //                     remaining
    //                 }
    //             };
    //         }
    //     }
    }
}
