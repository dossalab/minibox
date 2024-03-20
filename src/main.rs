#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt::info;
use embassy_executor::Spawner;
use embassy_nrf::{interrupt, Peripherals};
use embassy_time::Timer;

use git_version::git_version;

use defmt_rtt as _;
use nrf_softdevice as _;
use panic_probe as _;

fn embassy_init() -> Peripherals {
    let mut config = embassy_nrf::config::Config::default();

    /*
     * Softdevice implicitly utilizes the highest-level interrupt priority
     * We have to move all other interrupts to lower priority, unless
     * random issues and asserts from the Softdevice may (and will) occur
     */
    config.gpiote_interrupt_priority = interrupt::Priority::P2;
    config.time_interrupt_priority = interrupt::Priority::P2;

    return embassy_nrf::init(config);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_init();

    info!("A random Rust demo ({}) is running. Hello!", git_version!());

    loop {
        Timer::after_secs(3).await;
    }
}
