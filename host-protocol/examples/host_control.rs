use clap::{Parser, ValueEnum};
use crc::{Crc, CRC_32_ISCSI};
use host_protocol::{Bluetooth, Bootloader, HostProtocolMessage};
use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio_serial::SerialPortBuilderExt;

const CHUNK_SIZE: usize = 256;

#[derive(Clone, Debug, PartialEq, ValueEnum)]
enum Command {
    Reset,
    Enable,
    Disable,
    Rssi,
    Address,
    FwVersion,
    SendData,
    EraseApp,
    UpdateApp,
}

impl From<Command> for HostProtocolMessage<'_> {
    fn from(cmd: Command) -> Self {
        match cmd {
            Command::Reset => HostProtocolMessage::Reset,
            Command::Enable => HostProtocolMessage::Bluetooth(Bluetooth::Enable),
            Command::Disable => HostProtocolMessage::Bluetooth(Bluetooth::Disable),
            Command::Rssi => HostProtocolMessage::Bluetooth(Bluetooth::GetSignalStrength),
            Command::Address => HostProtocolMessage::Bluetooth(Bluetooth::GetBtAddress),
            Command::FwVersion => HostProtocolMessage::Bluetooth(Bluetooth::GetFirmwareVersion),
            Command::SendData => HostProtocolMessage::Bluetooth(Bluetooth::SendData(&[0x30; 200])),
            Command::EraseApp => HostProtocolMessage::Bootloader(Bootloader::EraseFirmware),
            Command::UpdateApp => HostProtocolMessage::Bootloader(Bootloader::EraseFirmware),
        }
    }
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long)]
    list_ports: bool,
    #[arg(short, long, default_value_t = String::from("/dev/ttyUSB0"))]
    port: String,
    #[arg(short, long, default_value_t = 460800)]
    baudrate: u32,
    #[arg(short, long, value_enum)]
    cmd: Option<Command>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let args = Args::parse();

    if args.list_ports {
        let ports = tokio_serial::available_ports().unwrap();
        let ports: Vec<String> = ports.into_iter().map(|p| p.port_name).collect();
        println!("List of available serial ports:");
        for port in ports {
            println!("- {}", port);
        }
        return Ok(());
    }

    if let Some(cmd) = args.cmd {
        let mut serial = tokio_serial::new(&args.port, args.baudrate).open_native_async()?;
        let mut buf = [0; 512]; // Buffer large enough for all messages
        let msg = postcard::to_slice_cobs::<HostProtocolMessage>(&cmd.clone().into(), &mut buf)?;
        println!(">>{:02x?}", msg);
        serial.write_all(msg).await?;
        serial.flush().await?;

        // Wait for response
        if let Err(_) = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::io::AsyncReadExt::read(&mut serial, &mut buf),
        )
        .await
        {
            println!("No response from device");
            return Ok(());
        }
        // Parse response
        let cobs_buf = buf.as_mut_slice();
        let ans: HostProtocolMessage = postcard::from_bytes_cobs(cobs_buf).unwrap();
        match ans {
            HostProtocolMessage::Bluetooth(Bluetooth::AckFirmwareVersion { version }) => {
                println!("Firmware version: {version}");
            }
            HostProtocolMessage::Bluetooth(Bluetooth::SignalStrength(rssi)) => {
                println!("RSSI: {rssi}");
            }
            HostProtocolMessage::Bluetooth(Bluetooth::AckBtAddress { bt_address }) => {
                println!("BT address: {:02x?}", bt_address);
            }
            HostProtocolMessage::Bootloader(Bootloader::AckEraseFirmware) => {
                println!("Erased Application firmware!");
                if cmd == Command::UpdateApp {
                    println!("Using file in BtPackage folder to update - BT_application_signed.bin ");
                    let update_file = include_bytes!("../../BtPackage/BT_application_signed.bin");
                    for app_chunk in update_file.chunks(CHUNK_SIZE).enumerate() {
                        // Prepare cobs packet to send to bootloader
                        let block = Bootloader::WriteFirmwareBlock {
                            block_idx: app_chunk.0,
                            block_data: app_chunk.1,
                        };
                        println!("Preparing chunk idx {}", app_chunk.0);
                        let crc = Crc::<u32>::new(&CRC_32_ISCSI);
                        let crc_pkt = crc.checksum(app_chunk.1);

                        let block_msg = postcard::to_slice_cobs(&HostProtocolMessage::Bootloader(block.clone()), &mut buf).unwrap();
                        serial.write_all(block_msg).await?;
                        serial.flush().await?;
                        match tokio::time::timeout(
                            std::time::Duration::from_millis(100),
                            tokio::io::AsyncReadExt::read(&mut serial, &mut buf),
                        )
                        .await
                        {
                            Ok(Ok(_)) => {
                                println!("Chunk {} sent!", app_chunk.0);
                                let cobs_buf = buf.as_mut_slice();
                                let msg: HostProtocolMessage = postcard::from_bytes_cobs(cobs_buf).unwrap();

                                match msg {
                                    HostProtocolMessage::Bootloader(Bootloader::AckWithIdxCrc { block_idx, crc }) => {
                                        if (block_idx == app_chunk.0) && (crc == crc_pkt) {
                                            println!("ACK packet {} with CRC {}", block_idx, crc);
                                        } else {
                                            println!("CRC mismatch");
                                            break;
                                        }
                                    }
                                    HostProtocolMessage::Bootloader(Bootloader::NackWithIdx { block_idx }) => {
                                        println!("Chunk {} not acknowledged!", block_idx);
                                        break;
                                    }
                                    _ => (),
                                }
                            }
                            Err(_) => {
                                println!("No response from device");
                                break;
                            }
                            _ => (),
                        }
                    }
                }
            }
            _ => {
                println!("<{ans:?}");
            }
        }
    } else {
        println!("Choose a command to be send.");
    }

    Ok(())
}
