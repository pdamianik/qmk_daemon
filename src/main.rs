pub mod qmk;
mod pipewire;

use hidapi::HidApi;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use log::{debug, LevelFilter};
use simple_logger::SimpleLogger;
use crate::pipewire::{listen_for_volume_change, VolumeInformation};
use crate::qmk::{show_volume, Filter};

const KEYCHRON: u16 = 0x3434;
const V3_MAX: u16 = 0x0934;

fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().with_level(LevelFilter::Error).env().init()?;

    let rx = Arc::new(Mutex::<Option<VolumeInformation>>::new(None));
    let tx = Arc::clone(&rx);

    thread::spawn(move || {
        let mut api = HidApi::new().unwrap();
        api.reset_devices().unwrap();
        api.add_devices(KEYCHRON, V3_MAX).unwrap();
        let devices = api.device_list().filter(Filter::None.filter())
            .map(|info| info.open_device(&api))
            .collect::<Result<Vec<_>, _>>().unwrap();

        loop {
            let value = rx.lock().unwrap().take();
            if let Some(Some((volume, muted))) = value {
                let level = (volume.powf(1.0 / 4.0) * 100.0) as u8;
                debug!("level: {}", volume.powf(1.0/4.0));
                for device in &devices {
                    show_volume(device, level, muted).unwrap();
                    sleep(Duration::from_millis(50));
                }
            }
        }
    });

    listen_for_volume_change(move |volume| {
        tx.lock().unwrap().replace(volume);
    })
}
