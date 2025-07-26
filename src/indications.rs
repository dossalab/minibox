use defmt::info;
use embassy_futures::select::{select, Either};
use embassy_nrf::gpio;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Ticker, Timer};

use crate::LedResources;

#[derive(Copy, Clone)]
pub enum IndicationStyle {
    Disabled,
    BlinkFast,
    BlinkSlow,
    PermanentHigh,
}

const BLINK_FAST_INTERVAL: Duration = Duration::from_millis(100);
const BLINK_SLOW_INTERVAL: Duration = Duration::from_millis(1000);

pub type LedIndicationsSignal = Signal<CriticalSectionRawMutex, IndicationStyle>;

#[embassy_executor::task]
pub async fn run(signal: &'static LedIndicationsSignal, res: LedResources) {
    info!("led indications running...");

    let mut led = gpio::Output::new(res.led, gpio::Level::Low, gpio::OutputDrive::Standard);

    let do_blinking = async |mut l: gpio::Output, d| {
        let mut ticker = Ticker::every(d);
        loop {
            l.set_high();
            Timer::after_millis(1).await;
            l.set_low();
            ticker.next().await;
        }
    };

    let mut do_indications = async |x| match x {
        IndicationStyle::Disabled => {
            led.set_low();
            futures::future::pending().await
        }
        IndicationStyle::PermanentHigh => {
            led.set_high();
            futures::future::pending().await
        }
        IndicationStyle::BlinkFast => do_blinking(led, BLINK_FAST_INTERVAL),
        IndicationStyle::BlinkSlow => todo!(),
    };

    let mut style = IndicationStyle::Disabled;
    loop {
        let f = select(signal.wait(), do_indications(style.clone())).await;
        match f {
            Either::First(input) => style = input,
            Either::Second(_) => (),
        }
    }
}
