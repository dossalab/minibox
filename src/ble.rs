use byteorder::{ByteOrder, LittleEndian};
use defmt::{error, info};
use nrf_softdevice::{
    ble::{
        self,
        central::{self, ConnectError},
        gatt_client::{self, DiscoverError},
        security::SecurityHandler,
        Address, AddressType, EncryptError, EncryptionInfo,
    },
    Softdevice,
};
use static_cell::StaticCell;

use crate::xboxhid::{ButtonFlags, JoystickData, XboxHidServiceClient, XboxHidServiceClientEvent};

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

async fn wait_connection(sd: &Softdevice, bonder: &'static Bonder) -> Result<(), BleError> {
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

    let client: XboxHidServiceClient = gatt_client::discover(&conn).await?;

    client.hid_report_cccd_write(true).await?;

    // let report_map = unwrap!(client.hid_report_map_read().await);
    // info!("report map is {:x}", report_map);

    gatt_client::run(&conn, &client, |event| match event {
        XboxHidServiceClientEvent::HidReportNotification(val) => {
            let button_mask = LittleEndian::read_u24(&val[13..16]);

            let x1 = LittleEndian::read_u16(&val[0..2]);
            let y1 = LittleEndian::read_u16(&val[2..4]);
            let x2 = LittleEndian::read_u16(&val[4..6]);
            let y2 = LittleEndian::read_u16(&val[6..8]);

            let t1 = LittleEndian::read_u16(&val[8..10]);
            let t2 = LittleEndian::read_u16(&val[10..12]);

            let jd = JoystickData {
                j1: (x1, y1),
                j2: (x2, y2),
                t1,
                t2,
                buttons: ButtonFlags::from_bits_truncate(button_mask),
            };

            info!("jd: {}", jd);
        }
    })
    .await;

    Ok(())
}

#[embassy_executor::task]
pub async fn run(sd: &'static Softdevice) {
    static BONDER: StaticCell<Bonder> = StaticCell::new();

    let bonder = BONDER.init(Bonder::default());

    loop {
        match wait_connection(sd, bonder).await {
            Err(e) => error!("unable to handle connection - {}", e),
            Ok(_) => {}
        }
    }
}
