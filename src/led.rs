use defmt::info;
use embassy_nrf::gpio;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer};

use crate::LedResources;

pub type LedIndicationsSignal = Signal<CriticalSectionRawMutex, u32>;

#[embassy_executor::task]
pub async fn handle_indications_task(signal: &'static LedIndicationsSignal, res: LedResources) {
    info!("led indications running...");
    let mut led = gpio::Output::new(res.led, gpio::Level::Low, gpio::OutputDrive::Standard);

    loop {
        let val = signal.wait().await;

        if val == 1 {
            for _ in 0..2 {
                led.set_high();
                Timer::after(Duration::from_millis(50)).await;
                led.set_low();
                Timer::after(Duration::from_millis(100)).await;
            }
        } else if val == 2 {
            led.set_high();
        }
    }
}
