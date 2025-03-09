use std::{fs::OpenOptions, io::Read, time::Duration};

use k_line::KLine;
use kwp2000::{
    client::Client,
    constants::{ServiceError, ServiceId},
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

    let mut client = match Client::new(Box::new(port)).security_access_timeout_bypass() {
        Ok(c) => c.programming_mode(Some(38400)).unwrap(),
        Err((c, e)) => {
            if let Error::UnexpectedResponse(Response::Error(ProcessError {
                service: ServiceId::SecurityAccess,
                error: ServiceError::ServiceNotSupported,
            })) = e
            {
                c.programming_mode(Some(38400)).unwrap()
            } else {
                panic!("{:?}", e);
            }
        }
    };

    println!("in programming mode");

    client.use_fastest_timing().unwrap();

    println!("using fast timing");

    let mut client = client.security_access_timeout_bypass().unwrap();

    println!("have pogramming security access");

    let mut file = OpenOptions::new()
        .read(true)
        .open("write_test.bin")
        .unwrap();

    let mut address = memory_layout::BASE_ADDRESS;
    for (n, size) in memory_layout::SECTORS.into_iter().enumerate() {
        println!(
            "starting sector {} of {} with size {}",
            n + 1,
            memory_layout::SECTORS.len(),
            size
        );

        let mut data_buf = Vec::with_capacity(size as usize);
        file.read_exact(&mut data_buf).unwrap();

        client.write_data_bosch(address, &data_buf, KEY).unwrap();

        address += size;
        println!("  done.")
    }

    client.disconnect().unwrap();

    println!("disconnected");

    Ok(())
}
