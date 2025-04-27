use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    hal::peripherals::Peripherals,
    nvs::EspDefaultNvsPartition,
    wifi::{ClientConfiguration, Configuration::Client, EspWifi},
};

use std::{thread::sleep, time::Duration};

#[toml_cfg::toml_config]
struct WifiConfig {
    #[default("")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_password: &'static str,
}

pub fn connect() -> EspWifi<'static> {
    log::info!("Connecting to wifi...");

    let event_loop = EspSystemEventLoop::take().unwrap();
    let nvs = EspDefaultNvsPartition::take().unwrap();
    let peripherals = Peripherals::take().unwrap();

    let mut wifi_driver = EspWifi::new(peripherals.modem, event_loop, Some(nvs)).unwrap();

    let client_config = ClientConfiguration {
        ssid: WIFI_CONFIG.wifi_ssid.try_into().unwrap(),
        password: WIFI_CONFIG.wifi_password.try_into().unwrap(),
        ..Default::default()
    };

    wifi_driver
        .set_configuration(&Client(client_config))
        .unwrap();

    wifi_driver.start().unwrap();

    wifi_driver.connect().unwrap();

    while !wifi_driver.is_connected().unwrap() {
        log::info!(
            "Waiting for wifi to connect... {} {}",
            WIFI_CONFIG.wifi_ssid,
            WIFI_CONFIG.wifi_password
        );
        sleep(Duration::from_secs(1));
    }

    log::info!(
        "Connected to wifi. IP: {:?}",
        wifi_driver.sta_netif().get_ip_info().unwrap()
    );

    wifi_driver
}
