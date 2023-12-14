use embassy_stm32::{bind_interrupts, peripherals, usart, usb};

bind_interrupts!(pub struct Irqs {
    USB_LP => usb::InterruptHandler<peripherals::USB>;
    USART1 => usart::InterruptHandler<peripherals::USART1>;
});
