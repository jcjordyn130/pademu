#![no_std]

use std::time::{Duration, Instant};

#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct GamepadState {
    // Buttons
    pub south: bool, pub east: bool, pub west: bool, pub north: bool,
    pub dpad_up: bool, pub dpad_down: bool, pub dpad_left: bool, pub dpad_right: bool,
    pub l1: bool, pub r1: bool, pub l3: bool, pub r3: bool,
    pub start: bool, pub select: bool, pub home: bool,
    pub l2: u8, pub r2: u8,
    pub left_stick_x: i16, pub left_stick_y: i16,
    pub right_stick_x: i16, pub right_stick_y: i16,

    // Battery state
    pub battery_level: u8, // 0-100 percentage
    pub battery_full: bool, // plugged in and >= 100%
    pub battery_charging: bool, // plugged in and <100%
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
    pub fn print_active_inputs(&self, last_state: &mut GamepadState, last_time: &mut Instant) {
        // Use an array/slice approach since we are in no_std, 
        // or just print directly if you remove no_std. 
        // Assuming std is available for your binary:
        extern crate std;
        use std::vec::Vec;

        // Check for significant changes
        // Filters out stick jitter
        let mut changed = false;
        
        // Analog sticc threshholds
        const STICK_DEADZONE: i16 = 1000;
        const TRIGGER_DEADZONE:  i16 = 3;

        // Check for sticc/analog trigger change
        if (self.left_stick_x.saturating_sub(last_state.left_stick_x)).abs() > STICK_DEADZONE ||
           (self.left_stick_y.saturating_sub(last_state.left_stick_y)).abs() > STICK_DEADZONE ||
           (self.right_stick_x.saturating_sub(last_state.right_stick_x)).abs() > STICK_DEADZONE ||
           (self.right_stick_y.saturating_sub(last_state.right_stick_y)).abs() > STICK_DEADZONE ||
           (self.l2 as i16 - last_state.l2 as i16).abs() > TRIGGER_DEADZONE ||
           (self.r2 as i16 - last_state.r2 as i16).abs() > TRIGGER_DEADZONE 
        {
            changed = true;
        }
        
        // Check for digital state change
        if self.home != last_state.home || self.select != last_state.select || self.start != last_state.start ||
           self.north != last_state.north || self.south != last_state.south || 
           self.east != last_state.east || self.west != last_state.west ||
           self.dpad_up != last_state.dpad_up || self.dpad_down != last_state.dpad_down || 
           self.dpad_left != last_state.dpad_left || self.dpad_right != last_state.dpad_right ||
           self.l1 != last_state.l1 || self.r1 != last_state.r1 || self.l3 != last_state.l3 || self.r3 != last_state.r3 ||
           self.battery_level != last_state.battery_level || self.battery_charging != last_state.battery_charging 
        {
            changed = true;
        }

        // 2. Enforce the Throttle
        let now = Instant::now();
        if !changed || now.duration_since(*last_time) < Duration::from_millis(50) {
            return; // Abort print if it's just sensor noise or firing too fast
        }

        // Output is approved, update trackers
        *last_state = self.clone();
        *last_time = now;

        // 3. Build the Output Array
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

        let bat_status = if self.battery_charging { "Charging" } else { "Discharging" };

        std::println!(
            "Bat: {}% ({}) | Btn: {:?} | L2: {:>3} R2: {:>3} | LS: ({:>6}, {:>6}) RS: ({:>6}, {:>6})",
            self.battery_level, bat_status,
            active, self.l2, self.r2,
            self.left_stick_x, self.left_stick_y,
            self.right_stick_x, self.right_stick_y
        );
    }
}