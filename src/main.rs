use std::time::Duration;

use clap::{Parser, Subcommand};
use rusb::{Context, Device, DeviceHandle, Error, Result, UsbContext};

const VID: u16 = 0x3554;
const PID: u16 = 0xf509;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Commands
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    Activate {
        #[arg()]
        count: u8,
    },
    Select {
        #[arg()]
        profile: u8,
    },
    Set {
        #[arg()]
        profile: u8,
        #[arg()]
        value: u16,
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut context = Context::new()?;
    let (mut device, mut handle) = match open_device(&mut context, VID, PID) {
        Ok(e) => e,
        Err(Error::NotFound) => {
            eprintln!("Device not found");
            std::process::exit(1);
        },
        Err(_) => {
            eprintln!("Failed to open USB device");
            std::process::exit(1);
        }
    };

    println!(
        "Mouse found on bus {:03} with device id {:03}",
        device.bus_number(),
        device.address()
    );

    let endpoints = find_readable_endpoints(&mut device)?;

    for endpoint in &endpoints {
        match handle.kernel_driver_active(endpoint.iface) {
            Ok(true) => {
                handle.detach_kernel_driver(endpoint.iface)?;
            }
            Ok(false) => (),
            Err(e) => {
                eprintln!("Failed to test if interface is claimed by kernel driver: {:}", e);
                std::process::exit(1);
            }
        };
    }

    let conf_endpoint = endpoints
        .first()
        .expect("No endpoints found on the device");

    configure_endpoint(&mut handle, &conf_endpoint)?;
    handle.claim_interface(1)?;

    match args.cmd {
        Commands::Activate { count } => {
            if count < 1 || count > 4 {
                eprintln!("Count must be in range [1;4]");
                std::process::exit(1);
            }

            set_profiles_count(&mut handle, count)?;
        },
        Commands::Select { profile } => {
            if profile > 3 {
                eprintln!("Profile must be in range [0;3]");
                std::process::exit(1);
            }

            switch_profile(&mut handle, profile)?;
        },
        Commands::Set { profile, value } => {
            if profile > 3 {
                eprintln!("Profile must be in range [0;3]");
                std::process::exit(1);
            }
            if value < 50 || value > 26000 {
                eprintln!("DPI value must be in range [50;26000] it will be rounded down to a multiple of 50");
                std::process::exit(1);
            }

            set_profile_dpi(&mut handle, profile, value)?;
        }
    }

    // cleanup after use
    println!("Releasing interfaces...");
    handle.release_interface(conf_endpoint.iface)?;
    handle.release_interface(1)?;

    for edp in find_readable_endpoints(&mut device).unwrap() {
        println!("Attaching kernel driver...");
        handle.attach_kernel_driver(edp.iface)?;
    }

    Ok(())
}

fn open_device<T: UsbContext>(
    context: &mut T,
    vid: u16,
    pid: u16,
) -> Result<(Device<T>, DeviceHandle<T>)> {
    let devices = match context.devices() {
        Ok(d) => d,
        Err(e) => return Err(e),
    };

    for device in devices.iter() {
        let device_desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Warning: Failed to get device descriptor: {}", e);
                continue;                
            },
        };

        if device_desc.vendor_id() == vid && device_desc.product_id() == pid {
            match device.open() {
                Ok(handle) => return Ok((device, handle)),
                Err(e) => {
                    eprintln!("Failed to open the device: {}", e);
                    continue;
                }
            }
        }
    }

    Err(Error::NotFound)
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
