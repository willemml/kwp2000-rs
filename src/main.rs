#![feature(iter_map_windows)]

use std::{fs::OpenOptions, io::Read, io::Write, time::Duration};

use k_line::KLine;
use kwp2000::{
    client::Client,
    constants::{ServiceError, ServiceId},
    raw_message::RawMessage,
    response::{ProcessError, Response},
};

pub mod bcb;
pub mod k_line;
pub mod kwp2000;

pub const KEY: &[u8; 6] = b"GEHEIM";

pub const INIT_ADDRESS: u8 = 0x01;

#[derive(Debug, Clone)]
pub struct MemoryLayout {
    pub base_address: u32,
    pub size: u32,
    pub sectors: Vec<u32>,
}

pub mod memory_layout {
    pub const BASE_ADDRESS: u32 = 8388608;
    pub const SIZE: u32 = 1048576;

    pub const SECTORS: [u32; 19] = [
        16384, 8192, 8192, 32768, 65536, 65536, 65536, 65536, 65536, 65536, 65536, 65536, 65536,
        65536, 65536, 65536, 65536, 65536, 65536,
    ];
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("new diagnostic mode not expected")]
    UnexpectedMode,
    #[error("response not expected for given command")]
    UnexpectedResponse(Response),
    #[error("command being processed does not match last command sent")]
    UnexpectedPending,
    #[error("unexpected value")]
    UnexpectedValue,
    #[error("service not implemented client side")]
    NotImplemented,
    #[error("specified length is bigger than received data")]
    NotEnoughData,
    #[error("given checksum does not match message contents")]
    InvalidChecksum,
    #[error("unknown service given")]
    InvalidService,
    #[error("unknown service error type")]
    InvalidServiceError,
    #[error("security timeout in effect")]
    SecurityTimout,
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "serialport")]
    #[error("serialport error")]
    SerialPort(#[from] serialport::Error),
}

fn main() -> Result<(), Error> {
    let mut port = serialport::new("/dev/ttyUSB0", 10400)
        .timeout(Duration::from_millis(4000)) // ecu P3 default is 5000, but I want a bit of leeway so I can close the session cleanly
        .flow_control(serialport::FlowControl::None)
        .open_native()
        .unwrap();

    port.init_kwp2000(INIT_ADDRESS)?;

    println!("init done");

    let mut client = Client::new(Box::new(port));

    client.diagnostic_mode().unwrap();

    println!("diagmode");

    let mut file = OpenOptions::new().create(true).write(true).open("mem")?;

    for i in 0..(0x380000 / 0x50) {
        let addr = 0x380000u32 + (0x50 * i);
        let bytes = addr.to_be_bytes();
        if let Ok(data) = client.dd_read_address(addr, 0x50) {
            file.write_all(&data)?;
            println!("0x{:06x}", addr);

            if data
                .into_iter()
                .map_windows(|[w0, w1, w2, w3]| {
                    w3 == &bytes[0] && w2 == &bytes[1] && w1 == &bytes[2] && w0 == &bytes[3]
                })
                .any(|b| b)
            {
                println!("  yay");
            }
        }
    }

    client.disconnect().unwrap();

    println!("disconnected");

    Ok(())
}
