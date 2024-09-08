pub mod qmk;
mod pipewire;

use hidapi::HidApi;
use simple_logger::SimpleLogger;
use std::error::Error;
use std::thread::sleep;
use std::time::Duration;
use log::debug;
use crate::pipewire::listen_for_volume_change;
use crate::qmk::{show_volume, Filter};

const KEYCHRON: u16 = 0x3434;
const V3_MAX: u16 = 0x0934;

fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().init()?;

    listen_for_volume_change(|volume| {
        if let Some(volume) = volume {
            let api = HidApi::new().unwrap();
            let devices = api.device_list().filter(Filter::Product(KEYCHRON, V3_MAX).filter())
                .map(|info| info.open_device(&api))
                .collect::<Result<Vec<_>, _>>().unwrap();

            let level = (volume.powf(1.0/4.0) * 100.0) as u8;
            debug!("level: {}", volume.powf(1.0/4.0));
            for device in &devices {
                show_volume(device, level).unwrap();
                sleep(Duration::from_millis(50));
            }
        }
    })
}
