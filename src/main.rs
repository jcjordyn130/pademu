mod core;
mod inputs;
mod outputs;
mod usb;

use std::fs::OpenOptions;
use std::thread;
use std::io::Write;

use flume::bounded;
use anyhow::{Context, Result};

use crate::core::{GamepadState, InputDriver, OutputEmulator};
use crate::inputs::ds4::Ds4Driver;
use crate::outputs::ds3::{Ds3Emulator, Ds3Ep0Handler, get_ds3_descriptors};

fn main() -> Result<()> {
    // Capacity 1 ensures we only ever send the freshest frame (zero latency)
    let (tx, rx) = bounded::<GamepadState>(1);

    let my_bt_mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]; // Spoofed Pi BT MAC

    // 1. Open EP0 (Must be mutable to write descriptors)
    let mut ep0 = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ffs-ds3/ep0")
        .context("CRITICAL: Cannot open EP0. Is configfs mounted?")?;

    // 2. Inject Descriptors into EP0 so the kernel creates EP1
    let descriptors = get_ds3_descriptors();
    crate::usb::ffs::write_descriptors(&mut ep0, &descriptors, 3)
        .context("CRITICAL: Failed to write FunctionFS descriptors to EP0")?;

    // 3. Start USB Control Thread (EP0)
    thread::spawn(move || {
        let handler = Ds3Ep0Handler { spoofed_mac: my_bt_mac };
        crate::usb::ffs::run_ep0_loop(ep0, handler);
    });

    // 4. Open and start EP1 Data Thread
    // (This file now exists because write_descriptors succeeded)
    let mut ep1 = OpenOptions::new()
        .write(true)
        .open("/dev/ffs-ds3/ep1")
        .context("CRITICAL: Cannot open EP1")?;

    thread::spawn(move || {
        let emulator = Ds3Emulator; // Stateless formatter
        let mut buf = [0u8; 64];
        let mut current_state = GamepadState::default();

        loop {
            if let Ok(new_state) = rx.try_recv() {
                current_state = new_state;
            }
            let len = emulator.build_report(&current_state, &mut buf);
            if ep1.write_all(&buf[..len]).is_err() {
                thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    });

    // 5. Main Input Loop (DS4)
    let mut input_driver = Ds4Driver::new("/dev/hidraw0")
        .context("CRITICAL: failed to open input driver!")?;

    // Rolling state window of the past two frames
    // Aids in debugging
    let mut state = GamepadState::default();
    let mut last_state = state;

    println!("Listening for DS4 input...");
    
    loop {
        if input_driver.read_input(&mut state) {
            if state != last_state {
                state.print_active_inputs();
                last_state = state;
            }
            
            let _ = tx.try_send(state);
        }
    }
}