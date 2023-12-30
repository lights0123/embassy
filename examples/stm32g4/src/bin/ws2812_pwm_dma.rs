// Configure TIM3 in PWM mode, and start DMA Transfer(s) to send color data into ws2812.
// We assume the DIN pin of ws2812 connect to GPIO PB4, and ws2812 is properly powered.
//
// The idea is that the data rate of ws2812 is 800 kHz, and it use different duty ratio to represent bit 0 and bit 1.
// Thus we can set TIM overflow at 800 kHz, and let TIM Update Event trigger a DMA transfer, then let DMA change CCR value,
// such that pwm duty ratio meet the bit representation of ws2812.
//
// You may want to modify TIM CCR with Cortex core directly,
// but according to my test, Cortex core will need to run far more than 100 MHz to catch up with TIM.
// Thus we need to use a DMA.
//
// This demo is a combination of HAL, PAC, and manually invoke `dma::Transfer`.
// If you need a simpler way to control ws2812, you may want to take a look at `ws2812_spi.rs` file, which make use of SPI.
//
// Warning:
// DO NOT stare at ws2812 directy (especially after each MCU Reset), its (max) brightness could easily make your eyes feel burn.

#![no_std]
#![no_main]

use cichlid::prelude::RainbowFill;
use cichlid::ColorRGB;
use embassy_executor::Spawner;
use embassy_stm32::gpio::OutputType;
use embassy_stm32::pac;
use embassy_stm32::time::khz;
use embassy_stm32::timer::simple_pwm::{PwmPin, SimplePwm};
use embassy_stm32::timer::{Channel, CountingMode};
use embassy_time::{Duration, Instant, Ticker, Timer};
use {defmt_rtt as _, panic_probe as _};

const LED_COUNT: usize = 5;
struct LedBuf([u16; LED_COUNT * 24 + 1]);
impl Default for LedBuf {
    fn default() -> Self {
        Self([0; LED_COUNT * 24 + 1])
    }
}

impl LedBuf {
    fn fill_from(&mut self, buf: &[ColorRGB; LED_COUNT], n0: u16, n1: u16) {
        let mut i = 0;
        let byte_buf = unsafe { core::slice::from_raw_parts(buf.as_ptr() as *const u8, buf.len() * 3) };
        for channel in byte_buf {
            for bit in 0..u8::BITS {
                self.0[i] = if channel & (0b1000_0000 >> bit) > 0 { n1 } else { n0 };
                i += 1;
            }
        }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();

    // set SYSCLK/HCLK/PCLK2 to 20 MHz, thus each tick is 0.05 us,
    // and ws2812 timings are integer multiples of 0.05 us
    {
        use embassy_stm32::rcc::*;
        use embassy_stm32::time::*;
        config.enable_debug_during_sleep = true;
        // Change this to `false` to use the HSE clock source for the USB. This example assumes an 8MHz HSE.
        const USE_HSI48: bool = true;

        let plldivq = if USE_HSI48 { None } else { Some(PllQ::DIV6) };

        config.rcc.pll = Some(Pll {
            source: PllSource::HSE(Hertz::mhz(8)),
            prediv_m: PllM::DIV2,
            mul_n: PllN::MUL85,
            div_p: None,
            div_q: plldivq,
            // Main system clock at 170 MHz
            div_r: Some(PllR::DIV2),
        });

        // config.rcc.ahb_pre = AHBPrescaler::DIV2; // 200 Mhz
        // config.rcc.apb1_pre = APBPrescaler::DIV2; // 100 Mhz
        // config.rcc.apb2_pre = APBPrescaler::DIV4; // 100 Mhz

        config.rcc.mux = ClockSrc::PLL;

        if USE_HSI48 {
            // Sets up the Clock Recovery System (CRS) to use the USB SOF to trim the HSI48 oscillator.
            config.rcc.clock_48mhz_src = Some(Clock48MhzSrc::Hsi48(Hsi48Config { sync_from_usb: true }));
        } else {
            config.rcc.clock_48mhz_src = Some(Clock48MhzSrc::PllQ);
        }
    }

    let mut dp = embassy_stm32::init(config);

    let mut ws2812_pwm = SimplePwm::new(
        dp.TIM1,
        Some(PwmPin::new_ch1(dp.PA8, OutputType::PushPull)),
        None,
        None,
        None,
        khz(800), // data rate of ws2812
        CountingMode::EdgeAlignedUp,
    );

    // construct ws2812 non-return-to-zero (NRZ) code bit by bit
    // ws2812 only need 24 bits for each LED, but we add one bit more to keep PWM output low
    let mut led_buf = LedBuf::default();
    let mut colors = [ColorRGB::Red; LED_COUNT];
    let max_duty = ws2812_pwm.get_max_duty();
    defmt::info!("max duty {}", max_duty);
    let n0 = 8 * max_duty / 25; // ws2812 Bit 0 high level timing
    let n1 = 2 * n0; // ws2812 Bit 1 high level timing
    led_buf.fill_from(&colors, n0, n1);

    let pwm_channel = Channel::Ch1;

    pac::TIM1
        .ccmr_output(pwm_channel.index() / 2)
        .modify(|v| v.set_ocpe(pwm_channel.index() % 2, true));
    ws2812_pwm.enable(pwm_channel);
    // make sure PWM output keep low on first start

    // ws2812_pwm.set_duty(pwm_channel, max_duty / 2);
    // loop {
    //     embassy_time::Timer::after_secs(100).await;
    //     // defmt::info!("bruh");
    // }

    // PAC level hacking, enable output compare preload
    // keep output waveform integrity
    {
        use embassy_stm32::dma::{Transfer, TransferOptions};

        // configure FIFO and MBURST of DMA, to minimize DMA occupation on AHB/APB
        let dma_transfer_option = TransferOptions::default();

        // flip color at 2 Hz
        let mut ticker = Ticker::every(Duration::from_millis(10));
        loop {
            // start PWM output
            let start_hue: u8 = (Instant::now().as_millis() / 20) as u8;
            // defmt::info!("start {}", start_hue);
            let hue_delta: u16 = 1 << 10;

            colors.rainbow_fill(start_hue, hue_delta);
            led_buf.fill_from(&colors, n0, n1);

            // PAC level hacking, enable timer-update-event trigger DMA
            pac::TIM1.dier().modify(|v| v.set_ude(true));

            unsafe {
                // defmt::info!("dmaing!");
                Transfer::new_write(
                    // with &mut, we can easily reuse same DMA channel multiple times
                    &mut dp.DMA2_CH5,
                    46,
                    &led_buf.0,
                    pac::TIM1.ccr(pwm_channel.index()).as_ptr() as *mut _,
                    dma_transfer_option,
                )
                .await;
                // defmt::info!("done dma!");

                // Turn off timer-update-event trigger DMA as soon as possible.
                // Then clean the FIFO Error Flag if set.
                pac::TIM1.dier().modify(|v| v.set_ude(false));
                // if pac::DMA1.isr(0).read().feif(2) {
                //     pac::DMA1.ifcr(0).write(|v| v.set_feif(2, true));
                // }

                // ws2812 need at least 50 us low level input to confirm the input data and change it's state
                Timer::after_micros(50).await;
            }

            // stop PWM output for saving some energy
            // ws2812_pwm.disable(pwm_channel);

            // wait until ticker tick
            ticker.next().await;
        }
    }
}
