use defmt::info;
use embassy_nrf::gpio;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Timer};

use crate::LedResources;

pub enum IndicationStyle {
    BlinkOnce,
    PermanentHigh,
}

pub type LedIndicationsSignal = Signal<CriticalSectionRawMutex, IndicationStyle>;

#[embassy_executor::task]
pub async fn run(signal: &'static LedIndicationsSignal, res: LedResources) {
    info!("led indications running...");

    let mut led = gpio::Output::new(res.led, gpio::Level::Low, gpio::OutputDrive::Standard);
    loop {
        match signal.wait().await {
            IndicationStyle::BlinkOnce => {
                led.set_high();
                Timer::after(Duration::from_millis(50)).await;
                led.set_low();
                Timer::after(Duration::from_millis(100)).await;
            }
            IndicationStyle::PermanentHigh => led.set_high(),
        };
    }
}
