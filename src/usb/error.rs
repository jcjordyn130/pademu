use thiserror::Error;

#[derive(Error, Debug)]
pub enum UsbError {
    // Automatically wraps and forwards std::io::Error
    #[error("USB filesystem I/O error")]
    Io(#[from] std::io::Error),

    #[error("Provided FunctionFS descriptors are malformed or invalid size")]
    InvalidDescriptor,

    #[error("Endpoint stalled during control request: {0}")]
    EndpointStalled(String),

    #[error("Unhandled USB control request: {b_request_type:#04X} {b_request:#04X}")]
    UnhandledRequest {
        b_request_type: u8,
        b_request: u8,
    },
}

// A convenient alias so you don't have to type std::result::Result<T, UsbError> everywhere
pub type Result<T> = std::result::Result<T, UsbError>;