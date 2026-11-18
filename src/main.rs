mod core;
mod inputs;
mod outputs;

use std::fs::OpenOptions;
use std::thread;
use std::io::Write;

use flume::bounded;

use anyhow::{Context, Result};

use crate::core::{GamepadState, InputDriver, OutputEmulator};
use crate::inputs::ds4::Ds4Driver;
use crate::outputs::ds3::{Ds3Emulator, run_ds3_ep0_thread};

fn main() -> Result<()> {
    // Capacity 1 ensures we only ever send the freshest frame (zero latency)
    let (tx, rx) = bounded::<GamepadState>(1);

    // Open USB dev node
    let my_bt_mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]; // Spoofed Pi BT MAC
    let ep0 = OpenOptions::new()
    .read(true)
    .write(true)
    .open("/dev/ffs-ds3/ep0")
    .context("CRITICAL: Cannot open EP0")?;

    // Start USB Control Thread (EP0)
    thread::spawn(move || run_ds3_ep0_thread(ep0, my_bt_mac));

    // Open and start EP1 thread
    let mut ep1 = OpenOptions::new()
    .write(true)
    .open("/dev/ffs-ds3/ep1")
    .context("CRITICAL: Cannot open EP1")?;

    thread::spawn(move || {
        let emulator = Ds3Emulator { spoofed_mac: my_bt_mac };
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

    // 3. Main Input Loop (DS4)
    let mut output_driver = Ds4Driver::new("/dev/hidraw0")
    .context("CRITICAL: failed to open output driver!")?;

    let mut state = GamepadState::default();
    println!("Listening for DS4 input...");
    
    loop {
        if output_driver.read_input(&mut state) {
            let _ = tx.try_send(state);
        }
    }
}