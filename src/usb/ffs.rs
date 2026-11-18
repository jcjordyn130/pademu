use std::io::{Read, Write};
use std::fs::File;
use std::mem;

// Pull in the kernel structs from the adjacent types.rs file
use super::types::{
    FfsDescHeaderV2, FfsStrings, UsbCtrlRequest, 
    FUNCTIONFS_DESCRIPTORS_MAGIC_V2, FUNCTIONFS_STRINGS_MAGIC
};

/// Trait for handling device-specific control transfers (e.g., PS3 0xF2/0xF5)
pub trait Ep0Handler {
    /// Return true if the request was handled, false to stall the endpoint
    fn handle_control_request(&mut self, req: &UsbCtrlRequest, ep0: &mut File) -> std::io::Result<bool>;
}

/// Writes the raw descriptors to EP0 to trigger the kernel to create EP1
pub fn write_descriptors(ep0: &mut File, descriptors: &[u8]) -> std::io::Result<()> {
    let total_len = mem::size_of::<FfsDescHeaderV2>() as u32 + (descriptors.len() as u32 * 2);

    let header = FfsDescHeaderV2 {
        magic: FUNCTIONFS_DESCRIPTORS_MAGIC_V2.to_le(),
        length: total_len.to_le(),
        flags: 3.to_le(), // FS + HS
        fs_count: 3.to_le(), // Interface, HID, EP1 IN
        hs_count: 3.to_le(),
    };

    let mut blob = Vec::new();
    blob.extend_from_slice(unsafe {
        std::slice::from_raw_parts(&header as *const _ as *const u8, mem::size_of::<FfsDescHeaderV2>())
    });
    
    // Append Full-Speed and High-Speed descriptors (identical for this device)
    blob.extend_from_slice(descriptors);
    blob.extend_from_slice(descriptors);

    ep0.write_all(&blob)?;

    // Write empty strings payload
    let strings = FfsStrings {
        magic: FUNCTIONFS_STRINGS_MAGIC.to_le(),
        length: (mem::size_of::<FfsStrings>() as u32).to_le(),
        str_count: 0,
        lang_count: 0,
    };
    
    ep0.write_all(unsafe {
        std::slice::from_raw_parts(&strings as *const _ as *const u8, mem::size_of::<FfsStrings>())
    })?;

    Ok(())
}

/// The generic FunctionFS EP0 event loop
pub fn run_ep0_loop(mut ep0: File, mut handler: impl Ep0Handler) {
    let mut event_buf = [0u8; 128];
    
    loop {
        if ep0.read(&mut event_buf).is_ok() {
            // FunctionFS event struct starts with a 32-bit event type.
            // Setup packets are type 2 (FUNCTIONFS_SETUP), and the UsbCtrlRequest starts at byte 4.
            let req_ptr = &event_buf[4] as *const _ as *const UsbCtrlRequest;
            let req = unsafe { &*req_ptr };

            // Pass to the console-specific handler. If it returns false, stall the endpoint.
            match handler.handle_control_request(req, &mut ep0) {
                Ok(true) => {}, // Handled successfully
                _ => {
                    // Read 0 bytes to stall the endpoint (rejects unsupported USB requests)
                    let mut stall_buf = [0u8; 0];
                    let _ = ep0.read(&mut stall_buf);
                }
            }
        }
    }
}