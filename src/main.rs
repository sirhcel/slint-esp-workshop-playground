use std::sync::mpsc;
use std::time::{Duration, Instant};

mod dht22;
mod esp32;

slint::include_modules!();

struct SensorData {
    temperature_celsius: f32,
    humidity_percent: f32,
    when: Duration,
}

fn dht_task(tx: mpsc::Sender<SensorData>) {
    // let delay = Delay::new_default();
    let start = Instant::now();
    let dht = dht22::DHT22::new(13);

    loop {
        match dht.read() {
            #[allow(unused_variables)]
            Ok((temperature, humidity)) => {
                let data = SensorData {
                    temperature_celsius: temperature,
                    humidity_percent: humidity,
                    when: Instant::now().duration_since(start),
                };
                tx.send(data).unwrap_or_else(|e| {
                    log::error!("Sending sensor data failed: {:?}", e);
                });
            }
            Err(e) => {
                log::error!("Error reading DHT22: {:?}", e);
            }
        }

        unsafe {
            esp_idf_svc::sys::vTaskDelay(2000 / 10);
        }
    }
}

fn main() {
    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    // Set the platform
    slint::platform::set_platform(esp32::EspPlatform::new()).unwrap();

    let (sensor_tx, sensor_rx) = mpsc::channel::<SensorData>();
    std::thread::spawn(move || dht_task(sensor_tx));

    // Finally, run the app!
    let ui = AppWindow::new().expect("Failed to load UI");
    let ui_handle = ui.as_weak();

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(2000),
        move || {
            let ui = ui_handle.unwrap();
            let model = ViewModel::get(&ui);
            match sensor_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(data) => {
                    let when = data.when.as_secs().to_string();
                    model.set_current(WeatherRecord {
                        temperature_celsius: data.temperature_celsius,
                        humidity_percent: data.humidity_percent,
                        timestamp: slint::SharedString::from(when),
                    });
                }
                Err(e) => {
                    log::error!("Receiving sensor data failed: {:?}", e);
                }
            }
        },
    );

    ui.run().unwrap();
}
