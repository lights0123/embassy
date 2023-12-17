use core::cell::Cell;

use defmt::Format;
use embassy_time::Instant;

#[derive(Format, Clone, Copy)]
pub enum ControllerState {
    Stopped,
    RemoteControl,
    Autonomous,
}
#[derive(Format, Clone, Copy)]
pub struct Controller {
    pub state: ControllerState,
    pub left: u16,
    pub right: u16,
    pub last_updated: Instant,
}

#[derive(Format, Clone, Copy)]
pub struct Computer {
    pub left: u16,
    pub right: u16,
    pub last_updated: Instant,
}

impl Default for Computer {
    fn default() -> Self {
        Self {
            left: 1500,
            right: 1500,
            last_updated: Instant::from_ticks(0),
        }
    }
}

#[derive(Default, Format, Clone)]
pub struct State {
    pub controller: Cell<Option<Controller>>,
    pub computer: Cell<Computer>,
}
