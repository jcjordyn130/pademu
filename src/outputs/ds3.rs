use crate::core::{GamepadState, OutputEmulator, SystemEvent};
use crate::usb::error::{Result, UsbError};
use crate::usb::types::UsbCtrlRequest;
use crate::usb::ffs::Ep0Handler;
use crate::usb::gadget::GadgetConfig;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::mpsc::Sender;

const DS3_CALIBRATION_DATA: [u8; 48] = [
    0xEF, 0x04, 0x00, 0x0B, 0x03, 0x01, 0xB0, 0x00, 0x00, 0x00, 0x08, 0x77,
    0x09, 0xC4, 0x08, 0x83, 0x09, 0x93, 0x08, 0x30, 0x08, 0x83, 0x08, 0x1B,
    0x09, 0x0D, 0x08, 0x51, 0x09, 0x95, 0x08, 0x32, 0x08, 0x7F, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

// Returns the USB Gadget configuration for the DS3
pub fn get_gadget_config() -> GadgetConfig {
    GadgetConfig {
        gadget_name: "ds3",
        vid: "0x054c",
        pid: "0x0268",
        bcd_device: "0x0100",
        bcd_usb: "0x0200",
        manufacturer: "Sony",
        product: "PLAYSTATION(R)3 Controller",
    }
}

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
    fn report_size(&self) -> usize { 64 }

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

        // Accel/gyro
        // The DS3 expects ~72 ticks per G for accel, and ~8 ticks per deg/s for gyro.
        // Apply the DS3's specific axis inversions here.
        let ds3_accel_x = (509.0 + (state.accel_x * 72.0)).clamp(0.0, 1023.0) as u16;
        let ds3_accel_y = (512.0 - (state.accel_y * 72.0)).clamp(0.0, 1023.0) as u16; 
        let ds3_accel_z = (489.0 - (state.accel_z * 72.0)).clamp(0.0, 1023.0) as u16; 
        let ds3_gyro_z  = (512.0 + (state.gyro_y * 8.0)).clamp(0.0, 1023.0) as u16; 

        // Pack into buffer as Big-Endian (10-bit values masked into 16-bits)
        buffer[41] = (ds3_accel_x >> 8) as u8 & 0x03;
        buffer[42] = (ds3_accel_x & 0xFF) as u8;

        buffer[43] = (ds3_accel_y >> 8) as u8 & 0x03;
        buffer[44] = (ds3_accel_y & 0xFF) as u8;

        buffer[45] = (ds3_accel_z >> 8) as u8 & 0x03;
        buffer[46] = (ds3_accel_z & 0xFF) as u8;

        buffer[47] = (ds3_gyro_z >> 8) as u8 & 0x03;
        buffer[48] = (ds3_gyro_z & 0xFF) as u8;

        // Bytes 29-31: USB Connection & Battery State
        buffer[29] = 0x02; // Status: Plugged in

        buffer[30] = if state.battery_full {
            0xEF // Fully charged
        } else if state.battery_charging {
            0xEE // Actively charging
        } else {
            match state.battery_level {
                86..=100 => 0x05, // 3 Bars
                61..=85  => 0x04, // 2 Bars
                36..=60  => 0x03, // 1 Bar
                16..=35  => 0x02, // Low / Flashing
                6..=15   => 0x01, // Dying
                _        => 0x00, // Shutdown
            }
        };

        buffer[31] = 0x12; // Connection: USB

        64
    }
}

/// Handler used by the generic EP0 thread for PS3 control transfers
pub struct Ds3Ep0Handler {
    pub spoofed_mac: [u8; 6],
    pub ef_data: [u8; 64],
    pub event_tx: Sender<SystemEvent>,
}

impl Ds3Ep0Handler {
    pub fn new(spoofed_mac: [u8; 6], event_tx: Sender<SystemEvent>) -> Self {
        let mut ef_data = [0u8; 64];
        // Initialize with RosettaPad's default EF header
        ef_data[1] = 0xEF; ef_data[2] = 0x04; ef_data[4] = 0x08; 
        ef_data[6] = 0x03; ef_data[7] = 0x01; ef_data[48] = 0x05;
        
        Self { spoofed_mac, ef_data, event_tx }
    }

    /// Populates IN reports (Device-to-Host). Returns the number of bytes written.
    /// Le sigh, of course Sony violates the USB HID standard.
    fn handle_in_report(&self, report_id: u8, data: &mut [u8]) -> usize {
        match report_id {
            0xEF => {
                // Crypto / Authentication Challenge
                // The PS3 sends a cryptographic challenge block via an OUT report, and 
                // expects the controller to echo back a modified response here. We just 
                // echo back the cached data, which is enough to satisfy the kernel.
                data.copy_from_slice(&self.ef_data);
                64
            },
            0xF2 => {
                // Device MAC Address Report
                // The PS3 asks the controller for its Bluetooth MAC address so it can 
                // pair and trust it for wireless play. We inject our spoofed MAC at offset 4.
                let mut f2 = [
                    0xF2, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x03, 0x50, 0x81, 0xD8, 0x01
                ];
                f2[4..10].copy_from_slice(&self.spoofed_mac);
                data[..16].copy_from_slice(&f2);
                16
            },
            0xF5 => {
                // Host MAC Address Acknowledgment
                // After the PS3 sends its own MAC address to the controller, it asks 
                // for this report to confirm the controller received it. We inject 
                // our spoofed MAC at offset 2 here as the controller's signature.
                let mut f5 = [
                    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0xAE, 0x60, 0x00, 0x03, 0x50, 0x81, 0xD8, 0x01
                ];
                f5[2..8].copy_from_slice(&self.spoofed_mac);
                data[..16].copy_from_slice(&f5);
                16
            },
            0xF7 => {
                // Initial Calibration / Power Status
                // Sent during the handshake. Harcoded to a standard plugged-in state 
                // so the PS3 doesn't think the controller is immediately dying.
                data[..16].copy_from_slice(&[
                    0x02, 0x01, 0xF8, 0x02, 0x07, 0x02, 0xEF, 0xFF,
                    0x14, 0x33, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
                ]);
                16
            },
            0xF8 => {
                // LED / Rumble Operational Configuration
                // Another handshake report. Hardcoded to a neutral resting state to 
                // clear any persistent LEDs or rumble states on boot.
                data[..16].copy_from_slice(&[
                    0x00, 0x02, 0x00, 0x00, 0x08, 0x00, 0x03, 0x01,
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
                ]);
                16
            },
            0x01 => {
                // Initial Input Report (EP0)
                // The PS3 asks for the current state of buttons/sticks directly over the 
                // control endpoint (EP0) before the fast EP1 data polling stream kicks in. 
                // This is a hardcoded neutral state (no buttons pressed, sticks centered).
                data[..16].copy_from_slice(&[
                    0x00, 0x01, 0x04, 0x00, 0x08, 0x0C, 0x01, 0x02,
                    0x18, 0x18, 0x18, 0x18, 0x09, 0x0A, 0x10, 0x11
                ]);
                16
            },
            _ => {
                // Standard HID compliance: if an unknown report is requested, 
                // the first byte must always be the report ID itself.
                data[0] = report_id; 
                1
            }
        }
    }

    /// Parses OUT reports (Host-to-Device). Returns nothing, but may fire SystemEvents.
    fn handle_out_report(&mut self, b_req: u8, report_id: u8, data: &[u8]) {
        if b_req != 0x09 { return; } // We only care about SET_REPORT (0x09)

        match report_id {
            0xEF => {
                // Cache the PS3's EF challenge
                self.ef_data[0] = 0xEF;
                let copy_len = std::cmp::min(data.len(), 63);
                self.ef_data[1..1+copy_len].copy_from_slice(&data[..copy_len]);
            },
            0xF5 if data.len() >= 8 => {
                println!("PS3 Authentication: Received Host MAC {:02X?}", &data[2..8]);
            },
            0xF4 if data.len() >= 4 => {
                if data[0] == 0x42 && data[1] == 0x0B {
                    println!("PS3 Command: Standby/Shutdown");
                    let _ = self.event_tx.send(SystemEvent::Standby);
                } else if data[0] == 0x42 && data[1] == 0x0C {
                    println!("PS3 Command: Enable Reporting");
                }
            },
            0x01 if data.len() >= 6 => {
                let weak_motor = data[3];   
                let strong_motor = data[5]; 
                let ds3_leds = if data.len() >= 11 { data[10] } else { 0 };

                if strong_motor > 0 || weak_motor > 0 {
                    println!("PS3 Rumble -> L:{} R:{} | LEDs: 0x{:02X}", strong_motor, weak_motor, ds3_leds);
                }

                // Send the payload to the main thread over the event bus
                //let _ = self.event_tx.send(SystemEvent::Rumble { 
                //    strong: strong_motor, 
                //    weak: weak_motor, 
                //   leds: ds3_leds 
                //});
            },
            _ => {} // Ignore unknown OUT reports
        }
    }

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
    
    fn handle_setup(&mut self, ep0: &mut std::fs::File, bm_req: u8, b_req: u8, w_value: u16, w_length: u16) {
        let is_in = (bm_req & 0x80) != 0;
        let report_id = (w_value & 0xFF) as u8;

        if !is_in {
            // OUT Transfer (Host -> Device)
            if w_length > 0 {
                let mut data = vec![0u8; w_length as usize];
                if ep0.read_exact(&mut data).is_ok() {
                    self.handle_out_report(b_req, report_id, &data);
                }
            } else {
                let _ = ep0.read(&mut [0u8; 0]); 
            }
            
            // FFS requires acknowledging the status stage with a zero-byte write
            let _ = ep0.write(&[]);
            
        } else {
            // IN Transfer (Device -> Host)
            let mut data = vec![0u8; w_length as usize];
            
            if bm_req == 0xA1 && b_req == 0x01 && w_length > 0 {
                // Buffer sizing ensures we don't panic if w_length is weirdly short
                let mut temp_buf = [0u8; 64];
                let bytes_written = self.handle_in_report(report_id, &mut temp_buf);
                
                let copy_len = std::cmp::min(w_length as usize, bytes_written);
                data[..copy_len].copy_from_slice(&temp_buf[..copy_len]);
            }
            
            let _ = ep0.write_all(&data);
        }
    }
}