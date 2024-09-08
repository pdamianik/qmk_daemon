pub mod qmk;
mod pipewire;

use hidapi::HidApi;
use simple_logger::SimpleLogger;
use std::error::Error;
use std::sync::mpsc::{sync_channel, TrySendError};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use log::debug;
use crate::pipewire::{listen_for_volume_change, VolumeInformation};
use crate::qmk::{show_volume, Filter};

const KEYCHRON: u16 = 0x3434;
const V3_MAX: u16 = 0x0934;

fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new().init()?;

    let (tx, rx) = sync_channel::<VolumeInformation>(1);

    thread::spawn(move || {
        let mut api = HidApi::new().unwrap();
        api.reset_devices().unwrap();
        api.add_devices(KEYCHRON, V3_MAX).unwrap();
        let devices = api.device_list().filter(Filter::None.filter())
            .map(|info| info.open_device(&api))
            .collect::<Result<Vec<_>, _>>().unwrap();

        loop {
            if let Some((volume, muted)) = rx.recv().unwrap() {
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
        match tx.try_send(volume) {
            Err(TrySendError::Full(_value)) => (),
            value => value.unwrap(),
        }
    })
}
