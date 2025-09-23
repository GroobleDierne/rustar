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
    let (device, mut handle) = match open_device(&mut context, VID, PID) {
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

    println!("Claiming interfaces...");
    // Detach from interfaces
    handle.detach_kernel_driver(0)?;
    handle.detach_kernel_driver(1)?;
    // Claim interfaces
    handle.claim_interface(0)?;
    handle.claim_interface(1)?;

    match args.cmd {
        Commands::Activate { count } => {
            if !(1..4).contains(&count) {
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
            if !(50..26000).contains(&value) {
                eprintln!("DPI value must be in range [50;26000] it will be rounded down to a multiple of 50");
                std::process::exit(1);
            }

            set_profile_dpi(&mut handle, profile, value)?;
        }
    }

    // cleanup after use
    println!("Releasing interfaces...");
    // Only release the interfaces we claimed
    handle.release_interface(0)?;
    handle.release_interface(1)?;
    // Reattach borrowed interfaces
    handle.attach_kernel_driver(0)?;
    handle.attach_kernel_driver(1)?;

    Ok(())
}

fn open_device<T: UsbContext>(
    context: &mut T,
    vid: u16,
    pid: u16,
) -> Result<(Device<T>, DeviceHandle<T>)> {
    let devices = context.devices()?;

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

// profile must be in range [0;3] TODO get how many profiles are active from the mouse
fn switch_profile<T: UsbContext>(handle: &mut DeviceHandle<T>, profile: u8) -> Result<usize> {
    let data: [u8; 17] = [
        0x08, 0x07, 0x00, 0x00, 0x04, 0x02, profile, 0x55 - profile, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xeb,
    ];

    write_set_report(handle, data)
}


// count must be in range [1;4]
fn set_profiles_count<T: UsbContext>(handle: &mut DeviceHandle<T>, count: u8) -> Result<usize> {
    let data: [u8; 17] = [
        0x08, 0x07, 0x00, 0x00, 0x02, 0x02, count, 0x55 - count, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xed,
    ];

    write_set_report(handle, data)
}

// profile must be in range [0;3]
fn set_profile_dpi<T: UsbContext>(handle: &mut DeviceHandle<T>, profile: u8, dpi: u16) -> Result<usize> {
    let dpi_index: u16 = (dpi / 50) - 1;
    let lo: u8 = dpi_index as u8 ;
    let hi: u8 = (dpi_index >> 8) as u8;
    let checksum = 0x155 - (0x13 + (0x0c + profile as u16 * 4) + 0x55);

    let data: [u8; 17] = [
        0x08, 0x07, 0x00, 0x00, 0x0c + profile * 4, 0x04, lo, lo, hi * 0x44, ((0x55 - 2*lo as i16  - 0x44*hi as i16) & 0xFF) as u8, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, checksum as u8
    ];

    write_set_report(handle, data)
}

fn write_set_report<T: UsbContext>(handle: &mut DeviceHandle<T>, data: [u8; 17]) -> Result<usize> {
    let timeout = Duration::from_secs(1);

    const REQUEST_TYPE: u8 = 0x21;
    const REQUEST: u8 = 0x09;
    const VALUE: u16 = 0x0208;
    const INDEX: u16 = 0x0001;

    handle.write_control(REQUEST_TYPE, REQUEST, VALUE, INDEX, &data, timeout)
}

fn read_interrupt<T: UsbContext>(handle: &mut DeviceHandle<T>, address: u8) -> Result<Vec<u8>> {
    let timeout = Duration::from_secs(1);
    let mut buf = [0u8; 64];

    handle
        .read_interrupt(address, &mut buf, timeout)
        .map(|_| buf.to_vec())
}
