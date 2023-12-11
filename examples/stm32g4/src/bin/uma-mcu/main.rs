#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use assign_resources::assign_resources;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::rcc::{Clock48MhzSrc, ClockSrc, Hsi48Config, Pll, PllM, PllN, PllQ, PllR, PllSource};
use embassy_stm32::time::Hertz;
use embassy_stm32::{peripherals, Config};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod computer_comms;
mod interrupts;
mod state;
mod uma_protocol;

assign_resources! {
    usb: UsbResources {
        PA12: PA12,
        PA11: PA11,
        USB: USB,
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let mut config = Config::default();

    // Change this to `false` to use the HSE clock source for the USB. This example assumes an 8MHz HSE.
    const USE_HSI48: bool = true;

    let plldivq = if USE_HSI48 { None } else { Some(PllQ::DIV6) };

    config.rcc.pll = Some(Pll {
        source: PllSource::HSE(Hertz(8_000_000)),
        prediv_m: PllM::DIV2,
        mul_n: PllN::MUL72,
        div_p: None,
        div_q: plldivq,
        // Main system clock at 144 MHz
        div_r: Some(PllR::DIV2),
    });

    config.rcc.mux = ClockSrc::PLL;

    if USE_HSI48 {
        // Sets up the Clock Recovery System (CRS) to use the USB SOF to trim the HSI48 oscillator.
        config.rcc.clock_48mhz_src = Some(Clock48MhzSrc::Hsi48(Hsi48Config { sync_from_usb: true }));
    } else {
        config.rcc.clock_48mhz_src = Some(Clock48MhzSrc::PllQ);
    }

    let p = embassy_stm32::init(config);
    let r = split_resources!(p);

    info!("Hello World!");
    static SHARED_STATE: StaticCell<state::State> = StaticCell::new();
    let shared_state = SHARED_STATE.init_with(Default::default);

    computer_comms::init_usb(r.usb, spawner, shared_state);
}
