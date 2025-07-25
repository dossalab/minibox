use defmt::{error, info};

use embassy_nrf::gpio::Pin;
use embassy_nrf::{bind_interrupts, gpio, peripherals, twim};
use embassy_time::{with_timeout, Duration, Timer};

use bq27xxx::defs::StatusFlags;
use bq27xxx::memory::memory_subclass;
use bq27xxx::{Bq27xx, ChemId, ChipError};

use crate::led::LedIndicationsSignal;
use crate::PmicResources;

type NrfBq27xxx<'a, I> = Bq27xx<twim::Twim<'a, I>, embassy_time::Delay>;
type NrfGaugeError = ChipError<twim::Error>;

async fn power_run<'a, I: twim::Instance>(
    signal: &'static LedIndicationsSignal,
    gauge: &mut NrfBq27xxx<'a, I>,
    pin: &gpio::AnyPin,
) -> Result<(), NrfGaugeError> {
    const INT_DEBOUNCE_MS: u64 = 5;
    let override_itpor = false;
    // let override_itpor = true;

    let mut int = gpio::Input::new(pin, gpio::Pull::Up);

    info!("detected battery gauge: {}", gauge.probe().await?);

    let flags = gauge.get_flags().await?;

    if override_itpor || flags.contains(StatusFlags::ITPOR) {
        info!("fuel gauge was reset, configuring...");

        let mut block = gauge.memblock_read(memory_subclass::STATE, 0).await?;

        info!("battery state block is {:02x}", block.raw);

        gauge.write_chem_id(ChemId::B4200).await?;

        // For 500 mAH
        // state_block.raw[6] = 0x01;
        // state_block.raw[7] = 0xf4;
        // state_block.raw[8] = 0x07;
        // state_block.raw[9] = 0x3a;

        // // Taper Rate = Design Capacity / (0.1 × taper current)
        // state_block.raw[21] = 0x00;
        // state_block.raw[22] = 0x0c;

        // design capacity = 180 mah
        block.raw[6] = 0x00;
        block.raw[7] = 180;

        // Design energy = capacoty * 3.7
        block.raw[8] = 0x02;
        block.raw[9] = 0x9a;

        // charge current = 100 ma
        // taper current = 0.1 * charge current + 15%
        //  Taper Rate = Design Capacity / (0.1 * Taper Current)
        block.raw[21] = 0x00;
        block.raw[22] = 150;

        gauge
            .memblock_write(memory_subclass::STATE, 0, &block)
            .await?;
    }

    info!("waiting for the fuel gauge events...");
    loop {
        let soc = gauge.state_of_charge().await?;
        let status = gauge.get_control_status().await?;
        let flags = gauge.get_flags().await?;
        let current = gauge.average_current().await?;
        let voltage = gauge.voltage().await?;

        info!(
            "state of charge is {}%, flags are {}, status flags {}, current is {} mA, voltage is {} mV",
            soc, flags, status, current, voltage
        );

        signal.signal(1);

        if flags.contains(StatusFlags::SOC1) {
            signal.signal(2);
        }

        Timer::after(Duration::from_millis(INT_DEBOUNCE_MS)).await;

        if let Err(_) = with_timeout(Duration::from_secs(10), int.wait_for_low()).await {
            info!("wait timed out")
        }
    }
}

#[embassy_executor::task]
pub async fn handle_power_task(signal: &'static LedIndicationsSignal, pmic: PmicResources) {
    const RETRY_RATE_SEC: u64 = 10;

    bind_interrupts!(struct Irqs {
        SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0 => twim::InterruptHandler<peripherals::TWISPI0>;
    });

    let config = twim::Config::default();
    let i2c = twim::Twim::new(pmic.i2c, Irqs, pmic.sda, pmic.scl, config);
    let mut gauge = Bq27xx::new(i2c, embassy_time::Delay, 0x55);

    let int = pmic.int.degrade();
    loop {
        if let Err(e) = power_run(&signal, &mut gauge, &int).await {
            error!(
                "error while executing the power task ({}) - retrying in {} seconds",
                e, RETRY_RATE_SEC
            );

            Timer::after(Duration::from_secs(RETRY_RATE_SEC)).await;
        }
    }
}
