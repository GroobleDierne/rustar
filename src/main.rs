use std::{env, time::Duration};

use rusb::{Context, Device, DeviceHandle, Result, UsbContext};

const VID: u16 = 0x3554;
const PID: u16 = 0xf509;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut context = Context::new()?;
    let (mut device, mut handle) =
        open_device(&mut context, VID, PID).expect("Failed to open USB device");

    println!(
        "Bus {:03} Device {:03}",
        device.bus_number(),
        device.address()
    );

    if args.len() == 1 {
        panic!("Please specify a profile number");
    }

    let profile: i8 = match args
        .get(1)
        .expect("Please specify a profile number")
        .as_ref()
    {
        "0" => 0,
        "1" => 1,
        _ => -1,
    };

    if profile == -1 {
        panic!("Invalid profile");
    }

    let endpoints = find_readable_endpoints(&mut device)?;
    let mut conf_endpoint: Option<Endpoint> = None;

    for endpoint in endpoints {
        if conf_endpoint.is_none() {
            conf_endpoint = Some(endpoint);
        }

        let has_kernel_driver = match handle.kernel_driver_active(endpoint.iface) {
            Ok(true) => {
                handle.detach_kernel_driver(endpoint.iface)?;
                true
            }
            Ok(false) => false,
            Err(e) => {
                println!("{:?}", e);
                false
            }
        };
        println!("has kernel driver? {}", has_kernel_driver);
    }

    if let Some(endpoint) = conf_endpoint {
        println!("Configuring...");
        configure_endpoint(&mut handle, &endpoint)?;
        handle.claim_interface(1)?;

        // set_profile_dpi(&mut handle, 1, 100)?;
        switch_profile(&mut handle, profile.try_into().unwrap())?;

        // cleanup after use
        println!("Releasing interface...");
        handle.release_interface(endpoint.iface)?;
        handle.release_interface(1)?;
        for edp in find_readable_endpoints(&mut device).unwrap() {
            println!("Attaching kernel...");
            handle.attach_kernel_driver(edp.iface)?;
        }
    }

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
                Err(e) => {
                    println!("{}", e);
                    continue;
                }
            }
        }
    }

    None
}

#[derive(Debug, Clone, Copy)]
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

        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                for endpoint_desc in interface_desc.endpoint_descriptors() {
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

fn configure_endpoint<T: UsbContext>(
    handle: &mut DeviceHandle<T>,
    endpoint: &Endpoint,
) -> Result<()> {
    handle.set_active_configuration(endpoint.config)?;
    handle.claim_interface(endpoint.iface)?;
    handle.set_alternate_setting(endpoint.iface, endpoint.setting)
}

// profile must be in range [0;3] TODO get how many profiles are active from the mouse
fn switch_profile<T: UsbContext>(handle: &mut DeviceHandle<T>, profile: u8) -> Result<usize> {
    let timeout = Duration::from_secs(1);

    const REQUEST_TYPE: u8 = 0x21;
    const REQUEST: u8 = 0x09;
    const VALUE: u16 = 0x0208;
    const INDEX: u16 = 0x0001;
    let data: [u8; 17] = [
        0x08, 0x07, 0x00, 0x00, 0x04, 0x02, profile, 0x55 - profile, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xeb,
    ];

    handle.write_control(REQUEST_TYPE, REQUEST, VALUE, INDEX, &data, timeout)
}


// count must be in range [1;4]
fn set_profiles_count<T: UsbContext>(handle: &mut DeviceHandle<T>, count: u8) -> Result<usize> {
    let timeout = Duration::from_secs(1);

    const REQUEST_TYPE: u8 = 0x21;
    const REQUEST: u8 = 0x09;
    const VALUE: u16 = 0x0208;
    const INDEX: u16 = 0x0001;
    let data: [u8; 17] = [
        0x08, 0x07, 0x00, 0x00, 0x02, 0x02, count, 0x55 - count, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xeb,
    ];

    handle.write_control(REQUEST_TYPE, REQUEST, VALUE, INDEX, &data, timeout)
}

// profile must be in range [0;3]
fn set_profile_dpi<T: UsbContext>(handle: &mut DeviceHandle<T>, profile: u8, dpi: u16) -> Result<usize> {
    let timeout = Duration::from_secs(1);

    const REQUEST_TYPE: u8 = 0x21;
    const REQUEST: u8 = 0x09;
    const VALUE: u16 = 0x0208;
    const INDEX: u16 = 0x0001;

    let dpi_index: u16 = (dpi / 50) - 1;
    let lo: u8 = dpi_index as u8 ;
    let hi: u8 = (dpi_index >> 8) as u8;
    let checksum = 0x155 - (0x13 + (0x0c + profile as u16 * 4) + 0x55);

    let data: [u8; 17] = [
        0x08, 0x07, 0x00, 0x00, 0x0c + profile * 4, 0x04, lo, lo, hi * 0x44, ((0x55 - 2*lo as i16  - 0x44*hi as i16) & 0xFF) as u8, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, checksum as u8
    ];
    handle.write_control(REQUEST_TYPE, REQUEST, VALUE, INDEX, &data, timeout)
}
