use crate::core::{GamepadState, OutputEmulator};
use std::fs::File;
use std::io::{Read, Write};

pub struct Ds3Emulator {
    /// The Bluetooth MAC address of the Pi (spoofed to look like a DS3)
    pub spoofed_mac: [u8; 6],
}

impl OutputEmulator for Ds3Emulator {
    fn report_size(&self) -> usize { 49 }

    fn build_report(&self, state: &GamepadState, buffer: &mut [u8]) -> usize {
        buffer.fill(0);
        buffer[0] = 0x01;

        if state.select { buffer[2] |= 0x01; }
        if state.l3 { buffer[2] |= 0x02; }
        if state.r3 { buffer[2] |= 0x04; }
        if state.start { buffer[2] |= 0x08; }
        if state.dpad_up { buffer[2] |= 0x10; }
        if state.dpad_right { buffer[2] |= 0x20; }
        if state.dpad_down { buffer[2] |= 0x40; }
        if state.dpad_left { buffer[2] |= 0x80; }

        if state.l2 > 0 { buffer[3] |= 0x01; }
        if state.r2 > 0 { buffer[3] |= 0x02; }
        if state.l1 { buffer[3] |= 0x04; }
        if state.r1 { buffer[3] |= 0x08; }
        if state.north { buffer[3] |= 0x10; }
        if state.east { buffer[3] |= 0x20; }
        if state.south { buffer[3] |= 0x40; }
        if state.west { buffer[3] |= 0x80; }

        if state.home { buffer[4] |= 0x01; }

        buffer[6] = ((state.left_stick_x as i32 + 32768) >> 8) as u8;
        buffer[7] = ((state.left_stick_y as i32 + 32768) >> 8) as u8;
        buffer[8] = ((state.right_stick_x as i32 + 32768) >> 8) as u8;
        buffer[9] = ((state.right_stick_y as i32 + 32768) >> 8) as u8;

        // Neutral Gyro/Accel
        buffer[41] = 0x02; buffer[42] = 0x00;
        buffer[43] = 0x02; buffer[44] = 0x00;
        buffer[45] = 0x02; buffer[46] = 0x00;
        buffer[47] = 0x02; buffer[48] = 0x00;

        49
    }
}

#[repr(C)]
struct UsbCtrlRequest {
    b_request_type: u8,
    b_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
}

/// Handles FunctionFS EP0 control transfers. 
/// Captures the PS3 Host MAC during 0xF5 so BlueZ can connect to it wirelessly.
pub fn run_ds3_ep0_thread(mut ep0: File, spoofed_mac: [u8; 6]) {
    let mut event_buf = [0u8; 128];
    
    loop {
        if let Ok(_) = ep0.read(&mut event_buf) {
            let req_ptr = &event_buf[4] as *const _ as *const UsbCtrlRequest;
            let req = unsafe { &*req_ptr };

            // PS3 asks for our MAC (0xF2 GET_REPORT)
            if req.b_request_type == 0xA1 && req.b_request == 0x01 && req.w_value == 0x03F2 {
                let mut reply = [0u8; 17];
                reply[0] = 0xF2;
                reply[1] = 0xFF;
                reply[3..9].copy_from_slice(&spoofed_mac); // Tell PS3 our BT MAC
                let _ = ep0.write_all(&reply);
            } 
            // PS3 tells us its MAC (0xF5 SET_REPORT)
            else if req.b_request_type == 0x21 && req.b_request == 0x09 && req.w_value == 0x03F5 {
                let mut payload = vec![0u8; req.w_length as usize];
                let _ = ep0.read_exact(&mut payload);
                
                if payload.len() >= 8 {
                    let ps3_host_mac = &payload[2..8];
                    println!("Received PS3 Host BT MAC: {:02X?}", ps3_host_mac);
                    // TRIGGER BLUETOOTH HANDOFF HERE (e.g. D-Bus call to BlueZ to trust/pair this MAC)
                }
            }
            // Stream Enable (0xF4 SET_REPORT)
            else if req.b_request_type == 0x21 && req.b_request == 0x09 && req.w_value == 0x03F4 {
                let mut payload = vec![0u8; req.w_length as usize];
                let _ = ep0.read_exact(&mut payload);
                println!("PS3 enabled polling stream.");
            } else {
                let mut stall_buf = [0u8; 0];
                let _ = ep0.read(&mut stall_buf); 
            }
        }
    }
}