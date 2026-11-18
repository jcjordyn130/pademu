use std::thread;
use std::{fs::File, time::Duration};
use std::io::Read;
use crate::core::{GamepadState, InputDriver};

pub struct Ds4Driver {
    file: File,
    pub is_standby: bool,
}

impl Ds4Driver {
    pub fn new(path: &str) -> std::io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self { file, is_standby: false})
    }
}

impl InputDriver for Ds4Driver {
    fn read_input(&mut self, state: &mut GamepadState) -> bool {
        if self.is_standby {
            thread::sleep(Duration::from_millis(100));
        }
        
        let mut buf = [0u8; 64];
        let bytes_read = self.file.read(&mut buf).unwrap_or(0);

        if bytes_read != 64 {
            println!("DEBUG: read {bytes_read} bytes from input! expected 64");
        }

        if bytes_read < 10 {
            return false;
        }

        // Determine if we are reading a USB or Bluetooth report
        let offset = if buf[0] == 0x01 {
            0 // USB mode
        } else if buf[0] == 0x11 {
            2 // Bluetooth mode shifts data right by 2 bytes
        } else {
            return false; // Not a standard input report
        };

        // Map sticks (0-255) to (-32768 to 32767)
        state.left_stick_x = ((buf[offset + 1] as i32 - 128) * 256) as i16;
        state.left_stick_y = ((buf[offset + 2] as i32 - 128) * 256) as i16;
        state.right_stick_x = ((buf[offset + 3] as i32 - 128) * 256) as i16;
        state.right_stick_y = ((buf[offset + 4] as i32 - 128) * 256) as i16;

        let b1 = buf[offset + 5];
        let dpad = b1 & 0x0F;
        
        // 8 is the neutral state for the D-Pad
        state.dpad_up = matches!(dpad, 0 | 1 | 7);
        state.dpad_right = matches!(dpad, 1 | 2 | 3);
        state.dpad_down = matches!(dpad, 3 | 4 | 5);
        state.dpad_left = matches!(dpad, 5 | 6 | 7);

        state.west = (b1 & 0x10) != 0;  // Square
        state.south = (b1 & 0x20) != 0; // Cross
        state.east = (b1 & 0x40) != 0;  // Circle
        state.north = (b1 & 0x80) != 0; // Triangle

        let b2 = buf[offset + 6];
        state.l1 = (b2 & 0x01) != 0;
        state.r1 = (b2 & 0x02) != 0;
        
        state.select = (b2 & 0x10) != 0;
        state.start = (b2 & 0x20) != 0;
        state.l3 = (b2 & 0x40) != 0;
        state.r3 = (b2 & 0x80) != 0;
        
        // PS Button is on the next byte
        state.home = (buf[offset + 7] & 0x01) != 0;

        // Analog triggers
        state.l2 = buf[offset + 8];
        state.r2 = buf[offset + 9];

        // Accel/gyro
        // Offset 13-18 is Gyro (Pitch, Yaw, Roll)
        // Offset 19-24 is Accel (X, Y, Z)
        
        let raw_gyro_y = i16::from_le_bytes([buf[offset + 15], buf[offset + 16]]);
        let raw_accel_x = i16::from_le_bytes([buf[offset + 19], buf[offset + 20]]);
        let raw_accel_y = i16::from_le_bytes([buf[offset + 21], buf[offset + 22]]);
        let raw_accel_z = i16::from_le_bytes([buf[offset + 23], buf[offset + 24]]);

        state.gyro_y = raw_gyro_y as f32 / 1024.0;
        state.accel_x = raw_accel_x as f32 / 8192.0;
        state.accel_y = raw_accel_y as f32 / 8192.0; 
        state.accel_z = raw_accel_z as f32 / 8192.0; 

        // Battery byte is at index 30 for USB, index 32 for BT
        let battery_byte = buf[offset + 30];
    
        // Lower 4 bits hold the charge capacity (0 to 10)
        let raw_capacity = battery_byte & 0x0F;
    
        // Bit 4 (0x10) is the USB cable detection flag
        let cable_plugged = (battery_byte & 0x10) != 0;

        // Scale the 0-10 capacity up to 0-100% for the emulator state
        state.battery_level = (raw_capacity as f32 / 10.0 * 100.0).min(100.0) as u8;
        state.battery_full = cable_plugged && raw_capacity >= 10;
        state.battery_charging = cable_plugged && raw_capacity < 10;
        
        true
    }

    fn standby(&mut self) {
        self.is_standby = true;
        println!("DS4: Entered standby mode!");
    }

    fn wakeup(&mut self) {
        self.is_standby = false;
        println!("DS4: Exit standby mode!");
    }
}