use defmt::info;
use embassy_nrf::{
    gpio,
    peripherals::PWM0,
    pwm::SimplePwm,
    saadc::{self, Saadc},
};
use embassy_time::{with_timeout, Duration, Timer};
use pid::Pid;

use crate::{board::*, BleMessageSignal, JoystickData};

struct Mixer<'a> {
    motors_pwm: SimplePwm<'a, PWM0>,
    tail_p: gpio::Output<'a>,
}

struct Stabilizer<'a> {
    adc: Saadc<'a, 1>,
    gyro_power: gpio::Output<'a>,
    offset: i16,
    pid: Pid<f32>,
}

impl<'a> Mixer<'a> {
    const MAX_DUTY: u16 = 1024;

    pub fn update(&mut self, data: JoystickData) {
        let rudder_scale = 4;

        // info!(
        //     "j1: {}, j2: {}, t1: {}, t2: {}, buttons: {}",
        //     data.j1, data.j2, data.t1, data.t2, data.buttons
        // );

        let mut rudder: i32 = 1024 - (data.j2.0 >> 5) as i32;
        let mut throttle: i32 = 1024 - (data.j1.1 >> 5) as i32;
        let tail: i32 = 1024 - (data.j2.1 >> 5) as i32;

        rudder = -rudder;
        if throttle < 80 {
            throttle = 0;
        }

        if i32::abs(rudder) < 100 {
            rudder = 0;
        }

        let mut r1: i32 = throttle - rudder / rudder_scale;
        let mut r2: i32 = throttle + rudder / rudder_scale;

        if r1 < 0 {
            r1 = 0;
        }

        if r2 < 0 {
            r2 = 0;
        }

        if r1 > Self::MAX_DUTY as i32 {
            r1 = Self::MAX_DUTY as i32;
        }

        if r2 > Self::MAX_DUTY as i32 {
            r2 = Self::MAX_DUTY as i32;
        }

        if tail > 0 {
            self.motors_pwm.set_duty(2, tail as u16);
            self.tail_p.set_high();
        } else {
            self.motors_pwm.set_duty(2, Self::MAX_DUTY - (-tail as u16));
            self.tail_p.set_low();
        }

        self.motors_pwm.set_duty(0, Self::MAX_DUTY - r1 as u16);
        self.motors_pwm.set_duty(1, Self::MAX_DUTY - r2 as u16);
    }

    /// Kill the motors
    pub fn disarm(&mut self) {}

    pub fn new(res: MotorResources) -> Self {
        let pwm = SimplePwm::new_3ch(res.pwm, res.rotor1, res.rotor2, res.tail_n);

        pwm.set_max_duty(Self::MAX_DUTY);
        pwm.set_prescaler(embassy_nrf::pwm::Prescaler::Div4);

        let tail_p = gpio::Output::new(res.tail_p, gpio::Level::Low, gpio::OutputDrive::Standard);

        Self {
            motors_pwm: pwm,
            tail_p,
        }
    }
}

impl<'a> Stabilizer<'a> {
    fn gyro_power_on(&mut self) {
        self.gyro_power.set_high();
    }

    async fn sample(&mut self) -> i16 {
        let mut buf = [0; 1];
        self.adc.sample(&mut buf).await;

        buf[0] + self.offset
    }

    async fn calibrate(&mut self) {
        // average out couple of samples, then introduce offset
        const SAMPLES: usize = 10;
        let mut average: i16 = 0;

        for i in 1..=SAMPLES {
            let sample = self.sample().await;
            average += (sample - average) / i as i16;
        }

        info!("calibrated offset - {}", -average);
        self.offset = -average;
    }

    async fn run(&mut self) {
        let sample = self.sample().await;
        // let output = self.pid.next_control_output(sample.into());

        // self.pid.setpoint(4.0);

        // info!("sample: {}, pid is {}", sample, output.output);

        Timer::after_millis(50).await;
    }

    fn new(res: GyroResources) -> Self {
        let mut config = saadc::Config::default();
        config.oversample = saadc::Oversample::OVER32X;

        let channel_config = saadc::ChannelConfig::differential(res.vref, res.vin);

        let mut pid = Pid::new(0.0, 255.0);
        pid.p(2.0, 255.0);
        // pid.i(10.0, 255.0);

        Self {
            adc: Saadc::new(res.adc, Irqs, config, [channel_config]),
            gyro_power: gpio::Output::new(
                res.enable,
                gpio::Level::Low,
                gpio::OutputDrive::Standard,
            ),
            offset: 0,
            pid,
        }
    }
}

#[embassy_executor::task]
pub async fn handle_controls(
    signal: &'static BleMessageSignal,
    motors: MotorResources,
    gyro: GyroResources,
) {
    // if no incoming data kill the motors
    const INCOMING_TIMEOUT: Duration = Duration::from_secs(1);

    let mut mixer = Mixer::new(motors);
    let mut stabilizer = Stabilizer::new(gyro);

    // let's wait for the input to stabilize
    stabilizer.gyro_power_on();
    Timer::after_millis(50).await;

    stabilizer.calibrate().await;

    info!("control loop is running");

    embassy_futures::join::join(
        async {
            loop {
                stabilizer.run().await
            }
        },
        async {
            loop {
                let incoming = with_timeout(INCOMING_TIMEOUT, signal.wait()).await;
                match incoming {
                    Ok(data) => mixer.update(data),
                    Err(_) => mixer.disarm(),
                };
            }
        },
    )
    .await;
}
