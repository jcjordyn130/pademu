use std::io::{Read, Write};
use std::fs::File;
use std::mem;

// Pull in the kernel structs from the adjacent types.rs file
use super::error::{Result, UsbError};
use super::types::{
    FfsDescHeaderV2, FfsStrings, UsbCtrlRequest, 
    FUNCTIONFS_DESCRIPTORS_MAGIC_V2, FUNCTIONFS_STRINGS_MAGIC
};

/// Trait for handling device-specific control transfers (e.g., PS3 0xF2/0xF5)
pub trait Ep0Handler {
    /// Return true if the request was handled, false to stall the endpoint
    fn handle_control_request(&mut self, req: &UsbCtrlRequest, ep0: &mut File) -> Result<bool>;
    fn handle_setup(&mut self, ep0: &mut std::fs::File, bm_req: u8, b_req: u8, w_value: u16, w_length: u16);
}

/// Writes the raw descriptors to EP0 to trigger the kernel to create EP1
pub fn write_descriptors(ep0: &mut File, descriptors: &[u8], descriptor_count: u32) -> Result<()> {
    // no empty descriptors
    if descriptors.is_empty() {
        return Err(UsbError::InvalidDescriptor);
    }

    // Bring up FFS request
    let total_len = mem::size_of::<FfsDescHeaderV2>() as u32 + (descriptors.len() as u32 * 2);

    let header = FfsDescHeaderV2 {
        magic: FUNCTIONFS_DESCRIPTORS_MAGIC_V2.to_le(),
        length: total_len.to_le(),
        flags: 3u32.to_le(), // FS + HS
        fs_count: descriptor_count.to_le(),
        hs_count: descriptor_count.to_le(),
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
    
    // Write the actual EP0 payload
    ep0.write_all(unsafe {
        std::slice::from_raw_parts(&strings as *const _ as *const u8, mem::size_of::<FfsStrings>())
    })?;

    Ok(())
}

/// The generic FunctionFS EP0 event loop
pub fn run_ep0_loop(mut ep0: File, mut handler: impl Ep0Handler, debug: bool) {
    let mut event_buf = [0u8; 128]; 
    
    loop {
        match ep0.read(&mut event_buf) {
            Ok(len) if len >= 12 => {
                // Debug printing
                let event_type = event_buf[8];
                
                match event_type {
                    0 => if debug { println!("EP0: BIND"); },
                    1 => if debug { println!("EP0: UNBIND"); },
                    2 => if debug { println!("EP0: ENABLE"); },
                    3 => if debug { println!("EP0: DISABLE"); },
                    4 => {
                        let bm_request_type = event_buf[0];
                        let b_request = event_buf[1];
                        let w_value = u16::from_le_bytes([event_buf[2], event_buf[3]]);
                        let w_length = u16::from_le_bytes([event_buf[6], event_buf[7]]);
                        
                        if debug {
                            println!("EP0: SETUP (bmReqType: {:#04x}, bReq: {:#04x}, wVal: {:#06x}, wLen: {})", 
                                bm_request_type, b_request, w_value, w_length);
                        }
                        
                        handler.handle_setup(&mut ep0, bm_request_type, b_request, w_value, w_length);
                    },
                    _ => if debug { println!("EP0: Unknown event ({})", event_type); },
                }
            }
            Ok(_) => {}, 
            Err(e) => {
                if debug { println!("EP0 read error: {}", e); }
                break;
            }
        }
    }
}