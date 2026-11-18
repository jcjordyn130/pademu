mod core;
mod inputs;
mod outputs;

use std::fs::OpenOptions;
use std::thread;
use std::io::Write;
use flume::bounded;

use crate::core::{GamepadState, InputDriver, OutputEmulator};
use crate::inputs::ds4::Ds4Driver;
use crate::outputs::ds3::{Ds3Emulator, run_ds3_ep0_thread};

fn main() {
    // Capacity 1 ensures we only ever send the freshest frame (zero latency)
    let (tx, rx) = bounded::<GamepadState>(1);

    // 1. Start USB Control Thread (EP0) for BT Handshake
    let my_bt_mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]; // Spoofed Pi BT MAC
    if let Ok(ep0) = OpenOptions::new().read(true).write(true).open("/dev/ffs-ds3/ep0") {
        thread::spawn(move || run_ds3_ep0_thread(ep0, my_bt_mac));
    }

    // 2. Start USB Data Thread (EP1)
    if let Ok(mut ep1) = OpenOptions::new().write(true).open("/dev/ffs-ds3/ep1") {
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
    }

    // 3. Main Input Loop (DS4)
    if let Ok(mut ds4) = Ds4Driver::new("/dev/hidraw0") {
        let mut state = GamepadState::default();
        println!("Listening for DS4 input...");
        
        loop {
            if ds4.read_input(&mut state) {
                let _ = tx.try_send(state);
            }
        }
    }
}