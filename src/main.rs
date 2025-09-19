use rusb::{Context, Device, DeviceHandle, Result, UsbContext};

const VID: u16 = 0x3554;
const PID: u16 = 0xf509;

fn main() -> Result<()> {
    for device in rusb::devices().unwrap().iter() {
        let device_desc = device.device_descriptor().unwrap();

        if device_desc.vendor_id() != 0x3554 {
            continue;
        }

        println!("Bus {:03} Device {:03} ID {:04x}:{:04x}",
            device.bus_number(),
            device.address(),
            device_desc.vendor_id(),
            device_desc.product_id());
        println!("Conf possible {:03} Packet size {:03}", device_desc.num_configurations(), device_desc.max_packet_size());
        let conf_desc = device.config_descriptor(0).unwrap();
        println!("Max power {:} Number of interfaces {:}", conf_desc.max_power(), conf_desc.num_interfaces());

        for interface in conf_desc.interfaces() {
            for descriptor in interface.descriptors() {
                println!("Class {:x} Endpoints {:x}", descriptor.class_code(), descriptor.num_endpoints());
                for endpoint in descriptor.endpoint_descriptors() {
                    let dir = match endpoint.direction() {
                        rusb::Direction::In => "IN",
                        rusb::Direction::Out => "OUT"
                    };
                    let usage = match endpoint.usage_type() {
                        rusb::UsageType::Data => "Data",
                        rusb::UsageType::Feedback => "Feedback",
                        rusb::UsageType::FeedbackData => "FeedbackData",
                        rusb::UsageType::Reserved => "Reserved",
                    };
                    let sync_type = match endpoint.sync_type() {
                        rusb::SyncType::NoSync => "NoSync",
                        rusb::SyncType::Asynchronous => "Async",
                        rusb::SyncType::Adaptive => "Adaptative",
                        rusb::SyncType::Synchronous => "Synchronous",
                    };
                    let transfer_type = match endpoint.transfer_type() {
                        rusb::TransferType::Control => "Control",
                        rusb::TransferType::Isochronous => "Isochronous",
                        rusb::TransferType::Bulk => "Bulk",
                        rusb::TransferType::Interrupt => "Interrupt",
                    };
                    println!("Add: {:b}, Dir: {:}, Usage: {:}, Type: {:}, Sync Type: {:}, Trans Type: {:}", endpoint.address(), dir, usage, endpoint.descriptor_type(), sync_type, transfer_type);
                }
            }
        }
    }
    let mut context = Context::new()?;
    let (mut device, mut handle) =
        open_device(&mut context, VID, PID).expect("Failed to open USB device");
    Ok(())
}

fn open_device<T: UsbContext>(
    context: &mut T,
    vid: u16,
    pid: u16,
) -> Option<(Device<T>, DeviceHandle<T>)> {
    let devices = match context.devices() {
        Ok(d) => d,
        Err(_) => return None,
    };

    for device in devices.iter() {
        let device_desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };

        if device_desc.vendor_id() == vid && device_desc.product_id() == pid {
            match device.open() {
                Ok(handle) => return Some((device, handle)),
                Err(_) => continue,
            }
        }
    }

    None
}

#[derive(Debug)]
struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8,
}

// returns all readable endpoints for given usb device and descriptor
fn find_readable_endpoints<T: UsbContext>(device: &mut Device<T>) -> Result<Vec<Endpoint>> {
    let device_desc = device.device_descriptor()?;
    let mut endpoints = vec![];
    for n in 0..device_desc.num_configurations() {
        let config_desc = match device.config_descriptor(n) {
            Ok(c) => c,
            Err(_) => continue,
        };
        // println!("{:#?}", config_desc);
        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                // println!("{:#?}", interface_desc);
                for endpoint_desc in interface_desc.endpoint_descriptors() {
                    // println!("{:#?}", endpoint_desc);
                    endpoints.push(Endpoint {
                        config: config_desc.number(),
                        iface: interface_desc.interface_number(),
                        setting: interface_desc.setting_number(),
                        address: endpoint_desc.address(),
                    });
                }
            }
        }
    }

    Ok(endpoints)
}
