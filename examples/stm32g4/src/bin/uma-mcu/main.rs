#![no_std]
#![no_main]

use assign_resources::assign_resources;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::rcc::{Clock48MhzSrc, ClockSrc, Hsi48Config, Pll, PllM, PllN, PllQ, PllR, PllSource};
use embassy_stm32::time::Hertz;
use embassy_stm32::{peripherals, Config};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

mod computer_comms;
mod config;
mod interrupts;
mod pwm_loop;
mod sbus_in;
mod sport;
mod state;
mod uma_protocol;

assign_resources! {
    usb: UsbResources {
        PA12: PA12,
        PA11: PA11,
        USB: USB,
    }
    out: OutResources {
        IWDG: IWDG,
        TIM1: TIM1,
        TIM2: TIM2,
        TIM3: TIM3,
        /// left
        left_motor: PA8,
        hbridge_left: PA9,
        hbridge_right: PA6,
        flywheel: PA5,
        /// waterblast
        waterblast: PB6,
        /// right
        ///
        /// maybe bridgeable to PA2 (LPUART TX)
        right_motor: PA1,
        /// bridged to PA3 (USART2 RX)
        sport_pwm_3b: PA4,
        /// bridged to BOOT0/PB8 (USART3 RX)
        pwm_4b: PB7,
        motor_enable: PB0,
    }
    sbus: SbusResources {
        sbus: PA10,
        usart: USART1,
        dma: DMA1_CH1,
    }
    sport: SportResources {
        sport: PA3,
        usart: USART2,
        dma_1: DMA1_CH2,
        dma_2: DMA1_CH3,
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
        mul_n: PllN::MUL85,
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
    spawner.must_spawn(sbus_in::do_status(r.sbus, shared_state));
    spawner.must_spawn(sport::do_status(r.sport, shared_state));
    spawner.must_spawn(pwm_loop::do_status(r.out, shared_state));
}
