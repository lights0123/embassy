use embassy_stm32::{bind_interrupts, peripherals, usb};

bind_interrupts!(pub struct Irqs {
    USB_LP => usb::InterruptHandler<peripherals::USB>;
});
