#![no_std]

#[derive(Default, Clone, Copy, Debug)]
pub struct GamepadState {
    pub south: bool, pub east: bool, pub west: bool, pub north: bool,
    pub dpad_up: bool, pub dpad_down: bool, pub dpad_left: bool, pub dpad_right: bool,
    pub l1: bool, pub r1: bool, pub l3: bool, pub r3: bool,
    pub start: bool, pub select: bool, pub home: bool,
    pub l2: u8, pub r2: u8,
    pub left_stick_x: i16, pub left_stick_y: i16,
    pub right_stick_x: i16, pub right_stick_y: i16,
}

pub trait InputDriver {
    /// Reads from the hardware (e.g., hidraw) and updates the generic state
    fn read_input(&mut self, state: &mut GamepadState) -> bool;
}

pub trait OutputEmulator {
    /// Expected size of the output report
    fn report_size(&self) -> usize;
    /// Formats the generic state into the console-specific byte array
    fn build_report(&self, state: &GamepadState, buffer: &mut [u8]) -> usize;
}