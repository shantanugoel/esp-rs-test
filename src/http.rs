use esp_idf_svc::{
    http::client::{Configuration, EspHttpConnection},
};

use embedded_svc::http::client::{Client as HttpClient, Method};

pub fn get_http_client() -> HttpClient<EspHttpConnection> {
    let config = Configuration {
        use_global_ca_store: true,
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    };
    let client = EspHttpConnection::new(&config).unwrap();
    HttpClient::wrap(client)
}

pub fn get(client: &mut HttpClient<EspHttpConnection>, url: &str) -> String {
    let headers = [("accept", "application/json")];
    let request = client.request(Method::Get, url.as_ref(), &headers).unwrap();
    let mut response = request.submit().unwrap();
    let mut buf = [0; 1024];
    let mut body = String::new();
    while let Ok(n) = response.read(&mut buf) {
        if n == 0 {
            break;
        }
        body.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
    body
}