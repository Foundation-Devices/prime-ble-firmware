use btleplug::api::bleuuid::BleUuid;
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral, ScanFilter, WriteType};
use btleplug::platform::Manager;
use clap::Parser;
use futures::stream::StreamExt;
use std::error::Error;
use uuid::Uuid;

#[derive(Debug, Parser)]
struct Args {
    #[arg(short, long)]
    list_adapters: bool,
    #[arg(short, long, default_value_t = String::from("hci0"))]
    adapter: String,
    #[arg(short, long)]
    enumerate: bool,
    #[arg(short, long)]
    write_data: bool,
}

const NUS_UUID: Uuid = Uuid::from_u128(consts::NUS_UUID);
const WRITE_CHARACTERISTIC_UUID: Uuid = Uuid::from_u128(0x6E400002_B5A3_F393_E0A9_E50E24DCCA9E);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let args = Args::parse();

    let manager = Manager::new().await?;
    let adapter_list = manager.adapters().await?;
    if adapter_list.is_empty() {
        eprintln!("No Bluetooth adapters found");
    }
    if args.list_adapters {
        println!("List of available bluetooth adapters:");
        for adapter in adapter_list.iter() {
            println!("- {}", adapter.adapter_info().await?);
        }
        return Ok(());
    }

    // connect to the wanted adapter or the first one by default
    let mut wanted_adapter = None;
    for adapter in adapter_list.clone() {
        if let Ok(info) = adapter.adapter_info().await {
            if info.contains(&args.adapter) {
                wanted_adapter = Some(adapter);
                println!("Wanted adapter found: {}", info);
                break;
            }
        }
    }
    let central = if wanted_adapter.is_some() {
        wanted_adapter.unwrap()
    } else {
        let first = adapter_list.into_iter().nth(0).unwrap();
        println!(
            "Wanted adapter not found, using first available one: {}",
            first.adapter_info().await?
        );
        first
    };
    println!("CentralState: {:?}", central.adapter_state().await.unwrap());

    // Each adapter has an event stream, we fetch via events(),
    // simplifying the type, this will return what is essentially a
    // Future<Result<Stream<Item=CentralEvent>>>.
    let mut events = central.events().await?;

    // start scanning for devices
    println!("Starting scan...");
    central.start_scan(ScanFilter { services: vec![NUS_UUID] }).await?;

    // Print based on whatever the event receiver outputs. Note that the event
    // receiver blocks, so in a real program, this should be run in its own
    // thread (not task, as this library does not yet use async channels).
    while let Some(event) = events.next().await {
        match event {
            CentralEvent::DeviceDiscovered(id) => {
                let peripheral = central.peripheral(&id).await?;
                let properties = peripheral.properties().await?;
                let name = properties.and_then(|p| p.local_name).unwrap_or_default();
                if !name.contains(consts::SHORT_NAME) {
                    continue;
                }
                println!("DeviceDiscovered: {}", name);
                if !peripheral.is_connected().await? {
                    println!("Connecting to peripheral {}...", name);
                    if let Err(err) = peripheral.connect().await {
                        eprintln!("Error connecting to peripheral, skipping: {}", err);
                        continue;
                    }
                }
            }
            CentralEvent::StateUpdate(state) => {
                println!("AdapterStatusUpdate {:?}", state);
            }
            CentralEvent::DeviceConnected(id) => {
                let peripheral = central.peripheral(&id).await?;
                let properties = peripheral.properties().await?;
                let name = properties.and_then(|p| p.local_name).unwrap_or_default();
                println!("DeviceConnected: {}", name);
                if !name.contains("Prime") {
                    continue;
                }
                peripheral.discover_services().await?;
                println!("Discover {} services...", name);
                if args.enumerate {
                    for service in peripheral.services() {
                        println!("Service UUID {}, primary: {}", service.uuid, service.primary);
                        for characteristic in service.characteristics {
                            println!("  {:?}", characteristic);
                        }
                    }
                }
                if args.write_data {
                    let chars = peripheral.characteristics();
                    let cmd_char = chars
                        .iter()
                        .find(|c| c.uuid == WRITE_CHARACTERISTIC_UUID)
                        .expect("Unable to find characterics");
                    let data = [8u8; 250];
                    println!("Writing data to {}...", name);
                    peripheral.write(&cmd_char, &data, WriteType::WithoutResponse).await?;
                }
                println!("Disconnecting from {}...", name);
                peripheral.disconnect().await.expect("Error disconnecting from BLE peripheral");
            }
            CentralEvent::DeviceDisconnected(id) => {
                let peripheral = central.peripheral(&id).await?;
                let properties = peripheral.properties().await?;
                let name = properties.and_then(|p| p.local_name).unwrap_or_default();
                println!("DeviceDisconnected: {}", name);
            }
            CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data } => {
                let peripheral = central.peripheral(&id).await?;
                let properties = peripheral.properties().await?;
                let name = properties.and_then(|p| p.local_name).unwrap_or_default();
                println!("ManufacturerDataAdvertisement: {}, {:?}", name, manufacturer_data);
            }
            CentralEvent::ServiceDataAdvertisement { id, service_data } => {
                let peripheral = central.peripheral(&id).await?;
                let properties = peripheral.properties().await?;
                let name = properties.and_then(|p| p.local_name).unwrap_or_default();
                println!("ServiceDataAdvertisement: {}, {:?}", name, service_data);
            }
            CentralEvent::ServicesAdvertisement { id, services } => {
                let peripheral = central.peripheral(&id).await?;
                let properties = peripheral.properties().await?;
                let name = properties.and_then(|p| p.local_name).unwrap_or_default();
                let services: Vec<String> = services.into_iter().map(|s| s.to_short_string()).collect();
                println!("ServicesAdvertisement: {}, {:?}", name, services);
            }
            _ => {}
        }
    }

    Ok(())
}
