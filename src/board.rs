use assign_resources::assign_resources;

use embassy_nrf::{bind_interrupts, peripherals, saadc};

bind_interrupts!(pub struct Irqs {
    SAADC => saadc::InterruptHandler;
});

assign_resources! {
    led: LedResources {
        led: P0_00
    },
    motors: MotorResources {
        rotor1: P0_01,
        rotor2: P0_02,
        tail_n: P0_03,
        tail_p: P0_04,
        pwm: PWM0
    },
    pmic: PmicResources {
        int: P0_06,
        sda: P0_07,
        scl: P0_08,
        i2c: TWISPI0,
    },
    switch: SwitchResources {
        switch: P0_05,
    },
    gyro: GyroResources {
        adc: SAADC,
        enable: P0_26,
        vin: P0_28,
        vref: P0_29,
    }
}
