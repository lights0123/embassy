#![no_std]
#![no_main]

use assign_resources::assign_resources;
use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Pull};
use embassy_stm32::rcc::{Clock48MhzSrc, ClockSrc, Hsi48Config, Pll, PllM, PllN, PllQ, PllR, PllSource};
use embassy_stm32::time::Hertz;
use embassy_stm32::{peripherals, Config};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

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
        TIM16: TIM16,
        /// left
        left_motor: PA8,
        hbridge_left: PA5,
        hbridge_right: PA6,
        pwm_1b: PA9,
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
async fn main(_spawner: Spawner) {
    let config = Config::default();

    let p = embassy_stm32::init(config);

    let mut relay = Output::new(p.PB0, Level::Low, embassy_stm32::gpio::Speed::Low);
    let mut input = Input::new(p.PA6, Pull::Down);
    loop {
        relay.set_level(input.get_level());
    }
}
