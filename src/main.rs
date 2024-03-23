#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use assign_resources::assign_resources;
use byteorder::{ByteOrder, LittleEndian};
use defmt::{bitflags, trace};
use defmt::{error, info, unwrap};
use embassy_executor::Spawner;
use embassy_nrf::peripherals;
use embassy_nrf::pwm::SimplePwm;
use embassy_nrf::{
    gpio::{self, AnyPin, Pin},
    interrupt, Peripherals,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use git_version::git_version;
use nrf_softdevice::{
    self as _,
    ble::{
        self,
        central::{self, ConnectError},
        gatt_client::{self, DiscoverError},
        security::SecurityHandler,
        Address, AddressType, EncryptError, EncryptionInfo,
    },
    gatt_client, Softdevice,
};

use defmt_rtt as _;
use panic_probe as _;

use static_cell::StaticCell;

bitflags! {
    pub struct ButtonFlags:u32 {
        const BUTTON_A = 1 << 0;
        const BUTTON_B = 1 << 1;
        const BUTTON_X = 1 << 3;
        const BUTTON_Y = 1 << 4;
        const BUTTON_LB = 1 << 6;
        const BUTTON_RB = 1 << 7;
        const BUTTON_ACTION_1 = 1 << 10;
        const BUTTON_MENU = 1 << 11;
        const BUTTON_XBOX = 1 << 12;
        const BUTTON_LEFT_STICK = 1 << 13;
        const BUTTON_RIGHT_STICK = 1 << 14;
        const BUTTON_ACTION_2 = 1 << 16;
    }
}

struct JoystickData {
    j1: (u16, u16),
    j2: (u16, u16),
    t1: u16,
    t2: u16,
    buttons: ButtonFlags,
}

type MySignal = Signal<CriticalSectionRawMutex, JoystickData>;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
enum BleError {
    Encryption(ble::EncryptError),
    ConnectError,
    DiscoveryError,
    WriteError(gatt_client::WriteError),
}

impl From<ConnectError> for BleError {
    fn from(_: ConnectError) -> Self {
        return Self::ConnectError;
    }
}

impl From<DiscoverError> for BleError {
    fn from(_: DiscoverError) -> Self {
        return Self::DiscoveryError;
    }
}

impl From<gatt_client::WriteError> for BleError {
    fn from(e: gatt_client::WriteError) -> Self {
        return Self::WriteError(e);
    }
}

pub struct Bonder {}

impl Default for Bonder {
    fn default() -> Self {
        Bonder {}
    }
}

impl SecurityHandler for Bonder {
    fn can_bond(&self, _conn: &nrf_softdevice::ble::Connection) -> bool {
        true
    }

    fn on_bonded(
        &self,
        _conn: &ble::Connection,
        _master_id: ble::MasterId,
        _key: EncryptionInfo,
        _peer_id: ble::IdentityKey,
    ) {
        info!("on_bonded is called!")
    }
}

#[gatt_client(uuid = "1812")]
struct HidServiceClient {
    #[characteristic(uuid = "2a4b", read)]
    hid_report_map: [u8; 64],

    #[characteristic(uuid = "2a4d", read, notify)]
    hid_report: [u8; 16],
}

assign_resources! {
    system: SystemResources {
        led: P0_00
    },
    motors: MotorResources {
        rotor1: P0_01,
        rotor2: P0_02,
        tail_n: P0_03,
        tail_p: P0_04,
        pwm: PWM0
    }
}

fn embassy_init() -> Peripherals {
    let mut config = embassy_nrf::config::Config::default();

    /*
     * Softdevice implicitly utilizes the highest-level interrupt priority
     * We have to move all other interrupts to lower priority, unless
     * random issues and asserts from the Softdevice may (and will) occur
     */
    config.gpiote_interrupt_priority = interrupt::Priority::P2;
    config.time_interrupt_priority = interrupt::Priority::P2;

    embassy_nrf::init(config)
}

fn softdevice_init() -> &'static Softdevice {
    info!("initializing softdevice...");

    let config = nrf_softdevice::Config::default();
    Softdevice::enable(&config)
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) -> ! {
    sd.run().await
}

async fn run<'a>(
    sd: &Softdevice,
    signal: &'static MySignal,
    bonder: &'static Bonder,
) -> Result<(), BleError> {
    let addr = &[&Address::new(
        AddressType::Public,
        [0x9a, 0x58, 0xd7, 0xd7, 0x6a, 0xf4],
    )];

    let mut config = central::ConnectConfig::default();
    config.scan_config.whitelist = Some(addr);

    let conn = central::connect_with_security(sd, &config, bonder).await?;
    match conn.encrypt() {
        Ok(_) => info!("connection encrypted!"),

        Err(EncryptError::PeerKeysNotFound) => {
            info!("no peer keys, request pairing");

            match conn.request_pairing() {
                Ok(_) => info!("pairing done"),
                Err(e) => error!("pairing not done {}", e),
            }
        }

        Err(e) => {
            error!("unable to encrypt the connection");
            return Err(BleError::Encryption(e));
        }
    };

    info!("connected!");

    let client: HidServiceClient = gatt_client::discover(&conn).await?;

    client.hid_report_cccd_write(true).await?;

    // let report_map = unwrap!(client.hid_report_map_read().await);
    // info!("report map is {:x}", report_map);

    gatt_client::run(&conn, &client, |event| match event {
        HidServiceClientEvent::HidReportNotification(val) => {
            let button_mask = LittleEndian::read_u24(&val[13..16]);

            let x1 = LittleEndian::read_u16(&val[0..2]);
            let y1 = LittleEndian::read_u16(&val[2..4]);
            let x2 = LittleEndian::read_u16(&val[4..6]);
            let y2 = LittleEndian::read_u16(&val[6..8]);

            let t1 = LittleEndian::read_u16(&val[8..10]);
            let t2 = LittleEndian::read_u16(&val[10..12]);

            trace!("button mask is {:x}", button_mask);

            let jd = JoystickData {
                j1: (x1, y1),
                j2: (x2, y2),
                t1,
                t2,
                buttons: ButtonFlags::from_bits_truncate(button_mask),
            };

            signal.signal(jd);
        }
    })
    .await;

    Ok(())
}
#[embassy_executor::task]
async fn handle_ble_out(signal: &'static MySignal, res: MotorResources) {
    let mut pwm = SimplePwm::new_2ch(res.pwm, res.rotor1, res.rotor2);

    const MAX_DUTY: u16 = 1024;

    pwm.set_max_duty(MAX_DUTY);
    pwm.set_prescaler(embassy_nrf::pwm::Prescaler::Div1);

    info!("bluetooth message handler is running");

    loop {
        let data = signal.wait().await;

        info!(
            "j1: {}, j2: {}, t1: {}, t2: {}, buttons: {}",
            data.j1, data.j2, data.t1, data.t2, data.buttons
        );

        pwm.set_duty(0, MAX_DUTY - data.t1);
        pwm.set_duty(1, MAX_DUTY - data.t2);
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_init();
    let r = split_resources!(p);

    info!(
        "Xbox controller demo ({}) is running. Hello!",
        git_version!()
    );

    static BLE_DATA_SIGNAL: MySignal = Signal::new();

    let sd = softdevice_init();

    unwrap!(spawner.spawn(softdevice_task(sd)));
    unwrap!(spawner.spawn(handle_ble_out(&BLE_DATA_SIGNAL, r.motors)));

    info!("Starting the main loop!");

    static BONDER: StaticCell<Bonder> = StaticCell::new();
    let bonder = BONDER.init(Bonder::default());

    loop {
        if let Err(err) = run(sd, &BLE_DATA_SIGNAL, bonder).await {
            error!("error while handling connections ({})", err);
        }
    }
}
