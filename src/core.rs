#![no_std]

#[derive(Default, Clone, Copy, Debug, PartialEq)]
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


impl GamepadState {
    /// Prints a clean, readable summary of currently active inputs
    pub fn print_active_inputs(&self) {
        // Use an array/slice approach since we are in no_std, 
        // or just print directly if you remove no_std. 
        // Assuming std is available for your binary:
        extern crate std;
        use std::vec::Vec;

        let mut active = Vec::new();
        
        if self.south { active.push("Cross(S)"); }
        if self.east { active.push("Circle(E)"); }
        if self.west { active.push("Square(W)"); }
        if self.north { active.push("Triangle(N)"); }
        
        if self.dpad_up { active.push("D-Up"); }
        if self.dpad_down { active.push("D-Down"); }
        if self.dpad_left { active.push("D-Left"); }
        if self.dpad_right { active.push("D-Right"); }
        
        if self.l1 { active.push("L1"); }
        if self.r1 { active.push("R1"); }
        if self.l3 { active.push("L3"); }
        if self.r3 { active.push("R3"); }
        
        if self.start { active.push("Start"); }
        if self.select { active.push("Select"); }
        if self.home { active.push("PS"); }

        std::println!(
            "Buttons: {:?} | L2: {:>3} R2: {:>3} | LS: ({:>6}, {:>6}) RS: ({:>6}, {:>6})",
            active, self.l2, self.r2,
            self.left_stick_x, self.left_stick_y,
            self.right_stick_x, self.right_stick_y
        );
    }
}