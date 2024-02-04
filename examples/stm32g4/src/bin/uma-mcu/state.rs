use core::cell::Cell;

use defmt::Format;
use embassy_time::Instant;

#[derive(Format, Clone, Copy, PartialEq, Eq)]
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
    pub hbridge: u16,
    pub waterblast: bool,
    pub last_updated: Instant,
}

#[derive(Format, Clone, Copy)]
pub struct Computer {
    pub left: u16,
    pub right: u16,
    pub hbridge: u16,
    pub waterblast: bool,
    pub last_updated: Instant,
}

impl Default for Computer {
    fn default() -> Self {
        Self {
            left: 1500,
            right: 1500,
            hbridge: 1500,
            waterblast: false,
            last_updated: Instant::from_ticks(0),
        }
    }
}

#[derive(Format, Clone, Copy)]
pub struct GpsStats {
    pub speed: u32,
    pub heading: u32,
    pub last_updated: Instant,
}

#[derive(Default, Format, Clone)]
pub struct FrskySensor {
    pub voltage: Cell<u32>,
    pub current: Cell<u32>,
    pub temp_1: Cell<u32>,
    pub temp_2: Cell<u32>,
}

#[derive(Default, Format, Clone)]
pub struct State {
    pub controller: Cell<Option<Controller>>,
    pub computer: Cell<Computer>,
    pub gps_stats: Cell<Option<GpsStats>>,
    pub sensor: FrskySensor,
}
