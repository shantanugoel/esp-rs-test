use std::{thread::sleep, time::Duration};
use esp_idf_svc::hal::prelude::*;
use slint::ToSharedString;
use serde_json::Value;

mod wifi;
mod slint_platform;
mod http;

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

    let window = MainWindow::new().unwrap();

    let window_handle = window.as_weak();
    let mut client = http::get_http_client();

    window.on_update_fact(move || {
        log::info!("Updating fact!!");
        let window = window_handle.upgrade().unwrap();
        window.set_fact("Hello World!!".to_shared_string());
        let body = http::get(&mut client, "https://api.chucknorris.io/jokes/random");
        log::info!("Body: {}", body);
        let v: Value = serde_json::from_str(&body).unwrap();
        window.set_fact(v["value"].to_string().to_shared_string());
    });

    let wifi = wifi::connect(p.modem);
    log::info!("Wifi connected!!: {:?}", wifi.sta_netif().get_ip_info());

    window.run().unwrap();


    slint::spawn_local(async move {
        for _ in 0..5 {
            timer.delay(5 * timer.tick_hz()).await.unwrap();
            log::info!("Waiting!!");
        }
    })
    .unwrap();

    loop {
        sleep(Duration::from_secs(1));
    }
}
