#![no_std]
#![no_main]

use assign_resources::assign_resources;

use core::panic::PanicInfo;
use embassy_executor::Spawner;
use embassy_nrf::{interrupt, peripherals, Peri, Peripherals};
use embassy_sync::signal::Signal;
use git_version::git_version;
use led::LedIndicationsSignal;
use nrf_softdevice::Softdevice;

use defmt::{info, unwrap};

mod ble;
mod led;
mod xboxhid;

use defmt_rtt as _;

assign_resources! {
    led: LedResources {
        led: P0_11
    },
    channels: ChannelResources {
        ch0: P0_00,
        ch1: P0_01,
        ch2: P0_02,
        ch3: P0_03,
        ch4: P0_04,
    },
    battery: BatteryResources {
        battery: P0_05
    },
}

// It's safer to reboot rather than hang
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    cortex_m::peripheral::SCB::sys_reset();
}

fn hw_init() -> (Peripherals, &'static Softdevice) {
    let mut config = embassy_nrf::config::Config::default();

    /*
     * Softdevice implicitly utilizes the highest-level interrupt priority
     * We have to move all other interrupts to lower priority, unless
     * random issues and asserts from the Softdevice may (and will) occur
     */
    config.gpiote_interrupt_priority = interrupt::Priority::P2;
    config.time_interrupt_priority = interrupt::Priority::P2;

    let p = embassy_nrf::init(config);
    let sd = Softdevice::enable(&nrf_softdevice::Config::default());

    (p, sd)
}

#[embassy_executor::task]
async fn run_softdevice(sd: &'static Softdevice) -> ! {
    sd.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let (p, sd) = hw_init();
    let r = split_resources!(p);

    info!("Minibox ({}) is running. Hello!", git_version!());

    static LED_INDICATIONS_SIGNAL: LedIndicationsSignal = Signal::new();

    unwrap!(spawner.spawn(led::run(&LED_INDICATIONS_SIGNAL, r.led)));
    unwrap!(spawner.spawn(ble::run(sd)));
    unwrap!(spawner.spawn(run_softdevice(sd)));

    LED_INDICATIONS_SIGNAL.signal(led::IndicationStyle::BlinkOnce);
}
