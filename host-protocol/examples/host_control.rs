use clap::{Parser, ValueEnum};
use host_protocol::{Bluetooth, HostProtocolMessage};
use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio_serial::SerialPortBuilderExt;

#[derive(Clone, Debug, PartialEq, ValueEnum)]
enum Command {
    Reset,
    Enable,
    Disable,
    Rssi,
    Address,
    FwVersion,
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
        let msg = postcard::to_slice_cobs::<HostProtocolMessage>(&cmd.into(), &mut buf)?;
        println!(">>{:02x?}", msg);
        serial.write_all(msg).await?;
    } else {
        println!("Choose a command to be send.");
    }

    Ok(())
}
