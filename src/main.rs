use std::{thread::sleep, time::Duration};
use esp_idf_svc::hal::prelude::*;

mod wifi;
mod slint_platform;


slint::include_modules!();

fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("Hello, world!");

    let p = Peripherals::take().unwrap();

    let touch_i2c = esp_idf_svc::hal::i2c::I2cDriver::new(
        p.i2c0,
        p.pins.gpio8,
        p.pins.gpio9,
        &esp_idf_svc::hal::i2c::config::Config::new().baudrate(400_000.Hz()),
    )
    .unwrap();

    slint_platform::init(touch_i2c);

    let mut timer =
        esp_idf_svc::hal::timer::TimerDriver::new(p.timer00, &Default::default()).unwrap();

    slint::spawn_local(async move {
        for _ in 0..5 {
            timer.delay(5 * timer.tick_hz()).await.unwrap();
            eprintln!("hello from future");
        }
    })
    .unwrap();

    MainWindow::new().unwrap().run().unwrap();

    let wifi = wifi::connect();

    loop {
        sleep(Duration::from_secs(1));
    }
}
