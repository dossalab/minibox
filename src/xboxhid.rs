// Xbox one controller hid defs

use defmt::bitflags;
use nrf_softdevice::gatt_client;

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

#[derive(defmt::Format)]
pub struct JoystickData {
    pub j1: (u16, u16),
    pub j2: (u16, u16),
    pub t1: u16,
    pub t2: u16,
    pub buttons: ButtonFlags,
}

#[gatt_client(uuid = "1812")]
pub struct XboxHidServiceClient {
    #[characteristic(uuid = "2a4b", read)]
    pub hid_report_map: [u8; 64],

    #[characteristic(uuid = "2a4d", read, notify)]
    pub hid_report: [u8; 16],
}
