use hidapi::{DeviceInfo, HidDevice, HidError};
use log::debug;
use thiserror::Error;

const USAGE_PAGE: u16 = 0xFF60;
const USAGE: u16 = 0x61;
const REPORT_LENGTH: usize = 32;

const CUSTOM_PROTOCOL_ID: u8 = b'A';

#[repr(u8)]
#[repr(C)]
#[derive(Copy, Clone)]
enum Command {
    SetVolume {
        #[allow(dead_code)]
        volume: u8,
        #[allow(dead_code)]
        muted: bool,
    } = 0x01,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct Wrapper {
    empty: u8,
    protocol: u8,
    command: Command,
}

#[repr(C)]
union Packet {
    command: Wrapper,
    data: [u8; REPORT_LENGTH + 1],
}

impl From<Command> for Packet {
    fn from(command: Command) -> Self {
        Self {
            command: Wrapper {
                empty: 0x00,
                protocol: CUSTOM_PROTOCOL_ID,
                command,
            },
        }
    }
}

/// Filter for specific devices
#[derive(Debug, Copy, Clone)]
pub enum Filter {
    /// Do not filter
    None,
    /// Filter by vendor id
    Vendor(u16),
    /// Filter by vendor id and device id
    Product(u16, u16),
}

impl Filter {
    pub fn filter(self) -> impl Fn(&&DeviceInfo) -> bool {
        move |info| {
            info.usage_page() == USAGE_PAGE
                && info.usage() == USAGE
                && match self {
                    Filter::None => true,
                    Filter::Vendor(vendor_id) => info.vendor_id() == vendor_id,
                    Filter::Product(vendor_id, product_id) => {
                        info.vendor_id() == vendor_id && info.product_id() == product_id
                    }
                }
        }
    }
}

#[derive(Error, Debug)]
pub enum VolumeError {
    #[error("Volume has to be between 0 and 100")]
    InvalidVolumeError,
    #[error("Failed to interact with device: {0}")]
    ReadError(#[from] HidError),
    #[error("Keyboard failed to indicate volume")]
    Unsuccessful,
}

pub fn show_volume(device: &HidDevice, level: u8, muted: bool) -> Result<(), VolumeError> {
    if level > 100 {
        return Err(VolumeError::InvalidVolumeError);
    }

    let packet: Packet = Command::SetVolume {
        volume: level,
        muted,
    }
    .into();
    let data = unsafe { packet.data };
    debug!(
        "Sending {data:?} to device {:#?}",
        device.get_device_info()?.path()
    );
    device.write(&data)?;

    let mut response = [0x00; REPORT_LENGTH];
    device.read(&mut response)?;

    debug!(
        "Received {response:?} from device {:#?}",
        device.get_device_info()?.path()
    );

    if response[0] != 0x01 {
        return Err(VolumeError::Unsuccessful);
    }

    Ok(())
}
