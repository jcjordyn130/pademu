use std::fs::File;
use std::io::Read;
use crate::core::{GamepadState, InputDriver};

pub struct Ds4Driver {
    file: File,
}

impl Ds4Driver {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self { file })
    }
}

impl InputDriver for Ds4Driver {
    fn read_input(&mut self, state: &mut GamepadState) -> bool {
        let mut buf = [0u8; 64];
        if self.file.read(&mut buf).unwrap_or(0) < 10 {
            return false;
        }

        // DS4 USB Report ID is 0x01
        if buf[0] == 0x01 {
            // Map sticks (0-255) to (-32768 to 32767)
            state.left_stick_x = ((buf[1] as i32 - 128) * 256) as i16;
            state.left_stick_y = ((buf[2] as i32 - 128) * 256) as i16;
            state.right_stick_x = ((buf[3] as i32 - 128) * 256) as i16;
            state.right_stick_y = ((buf[4] as i32 - 128) * 256) as i16;

            let b1 = buf[5];
            let dpad = b1 & 0x0F;
            state.dpad_up = matches!(dpad, 0 | 1 | 7);
            state.dpad_right = matches!(dpad, 1 | 2 | 3);
            state.dpad_down = matches!(dpad, 3 | 4 | 5);
            state.dpad_left = matches!(dpad, 5 | 6 | 7);

            state.west = (b1 & 0x10) != 0;  // Square
            state.south = (b1 & 0x20) != 0; // Cross
            state.east = (b1 & 0x40) != 0;  // Circle
            state.north = (b1 & 0x80) != 0; // Triangle

            let b2 = buf[6];
            state.l1 = (b2 & 0x01) != 0;
            state.r1 = (b2 & 0x02) != 0;
            state.l2 = buf[8];
            state.r2 = buf[9];

            state.select = (b2 & 0x10) != 0;
            state.start = (b2 & 0x20) != 0;
            state.l3 = (b2 & 0x40) != 0;
            state.r3 = (b2 & 0x80) != 0;
            state.home = (buf[7] & 0x01) != 0;

            return true;
        }
        false
    }
}