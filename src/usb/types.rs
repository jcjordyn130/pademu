/* Linux USB Bindings 
These come *directly* from the Linux source code licensed under the GPLv2
While Linus generally tries to avoid breaking userspace, there are no guarentees.
*/


// USB and FunctionFS Magic Numbers
pub const FUNCTIONFS_DESCRIPTORS_MAGIC_V2: u32 = 3;
pub const FUNCTIONFS_STRINGS_MAGIC: u32 = 2;

#[repr(C, packed)]
pub struct FfsDescHeaderV2 {
    pub magic: u32,
    pub length: u32,
    pub flags: u32,
    pub fs_count: u32,
    pub hs_count: u32,
}

#[repr(C, packed)]
pub struct FfsStrings {
    pub magic: u32,
    pub length: u32,
    pub str_count: u32,
    pub lang_count: u32,
}

/// Standard USB Setup Packet (Section 9.3 of USB Spec)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct UsbCtrlRequest {
    pub b_request_type: u8,
    pub b_request: u8,
    pub w_value: u16,
    pub w_index: u16,
    pub w_length: u16,
}