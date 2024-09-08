pub mod qmk;
mod pipewire;

use hidapi::HidApi;
use std::error::Error;
use std::sync::{Arc, Condvar, Mutex};
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

    let tx = Arc::new((Mutex::<VolumeInformation>::new(None), Condvar::new()));
    let rx = tx.clone();

    thread::spawn(move || {
        let mut api = HidApi::new().unwrap();
        api.reset_devices().unwrap();
        api.add_devices(KEYCHRON, V3_MAX).unwrap();
        let devices = api.device_list().filter(Filter::None.filter())
            .map(|info| info.open_device(&api))
            .collect::<Result<Vec<_>, _>>().unwrap();

        loop {
            let value = {
                let mut value = rx.0.lock().unwrap();
                while value.is_none() {
                    value = rx.1.wait(value).unwrap();
                }
                value.take().unwrap()
            };
            let (volume, muted) = value;
            let level = (volume.powf(1.0 / 4.0) * 100.0) as u8;
            debug!("level: {}", volume.powf(1.0/4.0));
            for device in &devices {
                show_volume(device, level, muted).unwrap();
                sleep(Duration::from_millis(50));
            }
        }
    });

    listen_for_volume_change(move |volume| {
        *tx.0.lock().unwrap() = volume;
        tx.1.notify_one();
    })
}
