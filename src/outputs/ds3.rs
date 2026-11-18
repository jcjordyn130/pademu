use crate::core::{GamepadState, OutputEmulator};
use crate::usb::error::{Result, UsbError};
use crate::usb::types::UsbCtrlRequest;
use crate::usb::ffs::Ep0Handler;
use std::fs::File;
use std::io::{Read, Write};

/// Returns the concatenated Interface, HID, and EP1 IN descriptors for the DS3
pub fn get_ds3_descriptors() -> Vec<u8> {
    let mut desc = Vec::new();
    
    // 1. Interface Descriptor (Class 3 = HID)
    desc.extend_from_slice(&[
        0x09, 0x04, 0x00, 0x00, 0x01, 0x03, 0x00, 0x00, 0x00,
    ]);

    // 2. HID Descriptor
    desc.extend_from_slice(&[
        0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22, 148, 0x00,
    ]);

    // 3. Endpoint 1 IN Descriptor
    desc.extend_from_slice(&[
        0x07, 0x05, 0x81, 0x03, 0x40, 0x00, 0x01,
    ]);

    desc
}

/// Stateless emulator used by the EP1 data thread
pub struct Ds3Emulator;

impl OutputEmulator for Ds3Emulator {
    fn report_size(&self) -> usize { 49 }

    fn build_report(&self, state: &GamepadState, buffer: &mut [u8]) -> usize {
        buffer.fill(0);
        buffer[0] = 0x01;

        // Byte 2: Digital D-Pad & Modifiers
        if state.select { buffer[2] |= 0x01; }
        if state.l3 { buffer[2] |= 0x02; }
        if state.r3 { buffer[2] |= 0x04; }
        if state.start { buffer[2] |= 0x08; }
        if state.dpad_up { buffer[2] |= 0x10; }
        if state.dpad_right { buffer[2] |= 0x20; }
        if state.dpad_down { buffer[2] |= 0x40; }
        if state.dpad_left { buffer[2] |= 0x80; }

        // Byte 3: Digital Face Buttons & Bumpers
        if state.l2 > 0 { buffer[3] |= 0x01; }
        if state.r2 > 0 { buffer[3] |= 0x02; }
        if state.l1 { buffer[3] |= 0x04; }
        if state.r1 { buffer[3] |= 0x08; }
        if state.north { buffer[3] |= 0x10; } // Triangle
        if state.east { buffer[3] |= 0x20; }  // Circle
        if state.south { buffer[3] |= 0x40; } // Cross
        if state.west { buffer[3] |= 0x80; }  // Square

        // Byte 4: PS Button
        if state.home { buffer[4] |= 0x01; }

        // Bytes 6-9: Analog Sticks
        buffer[6] = ((state.left_stick_x as i32 + 32768) >> 8) as u8;
        buffer[7] = ((state.left_stick_y as i32 + 32768) >> 8) as u8;
        buffer[8] = ((state.right_stick_x as i32 + 32768) >> 8) as u8;
        buffer[9] = ((state.right_stick_y as i32 + 32768) >> 8) as u8;

        // Bytes 14-25: Analog Pressure Values (0-255)
        buffer[14] = if state.dpad_up { 255 } else { 0 };
        buffer[15] = if state.dpad_right { 255 } else { 0 };
        buffer[16] = if state.dpad_down { 255 } else { 0 };
        buffer[17] = if state.dpad_left { 255 } else { 0 };
        buffer[18] = state.l2; // True analog trigger values
        buffer[19] = state.r2; // True analog trigger values
        buffer[20] = if state.l1 { 255 } else { 0 };
        buffer[21] = if state.r1 { 255 } else { 0 };
        buffer[22] = if state.north { 255 } else { 0 };
        buffer[23] = if state.east { 255 } else { 0 };
        buffer[24] = if state.south { 255 } else { 0 };
        buffer[25] = if state.west { 255 } else { 0 };

        // Neutral Gyro/Accel
        buffer[41] = 0x02; buffer[42] = 0x00;
        buffer[43] = 0x02; buffer[44] = 0x00;
        buffer[45] = 0x02; buffer[46] = 0x00;
        buffer[47] = 0x02; buffer[48] = 0x00;

        49
    }
}

/// Handler used by the generic EP0 thread for PS3 control transfers
pub struct Ds3Ep0Handler {
    pub spoofed_mac: [u8; 6],
}

impl Ep0Handler for Ds3Ep0Handler {
    fn handle_control_request(&mut self, req: &UsbCtrlRequest, ep0: &mut File) -> Result<bool> {
        // 0xF2 GET_REPORT: PS3 asks for our MAC
        if req.b_request_type == 0xA1 && req.b_request == 0x01 && req.w_value == 0x03F2 {
            let mut reply = [0u8; 17];
            reply[0] = 0xF2;
            reply[1] = 0xFF;
            reply[3..9].copy_from_slice(&self.spoofed_mac); 
            ep0.write_all(&reply)?; // Automatically converts to UsbError::Io
            return Ok(true);
        } 
        // 0xF5 SET_REPORT: PS3 tells us its MAC
        else if req.b_request_type == 0x21 && req.b_request == 0x09 && req.w_value == 0x03F5 {
            let mut payload = vec![0u8; req.w_length as usize];
            ep0.read_exact(&mut payload)?;
            
            if payload.len() >= 8 {
                let ps3_host_mac = &payload[2..8];
                println!("Received PS3 Host BT MAC: {:02X?}", ps3_host_mac);
                // TODO: D-Bus call to BlueZ to trust/pair this MAC
            }
            return Ok(true);
        }
        // 0xF4 SET_REPORT: PS3 enables polling stream
        else if req.b_request_type == 0x21 && req.b_request == 0x09 && req.w_value == 0x03F4 {
            let mut payload = vec![0u8; req.w_length as usize];
            ep0.read_exact(&mut payload)?;
            println!("PS3 enabled polling stream.");
            return Ok(true);
        }

        // We don't recognize this packet. Return our custom thiserror variant so 
        // the EP0 loop stalls the endpoint and logs the unknown request.
        Err(UsbError::UnhandledRequest {
            b_request_type: req.b_request_type,
            b_request: req.b_request,
        })
    }
}