use std::{fs::OpenOptions, io::Write, time::Duration};

use k_line::KLine;
use kwp2000::{
    Interface,
    client::Client,
    constants::{CompressionFormat, DiagnosticMode, EncryptionFormat},
    message::{Message, TransferType},
    response::Response,
};

pub mod k_line;
pub mod kwp2000;

mod memory_layout {
    pub const BASE_ADDRESS: u32 = 8388608;
    pub const SIZE: u32 = 1048576;

    pub const SECTORS: [u32; 19] = [
        16384, 8192, 8192, 32768, 65536, 65536, 65536, 65536, 65536, 65536, 65536, 65536, 65536,
        65536, 65536, 65536, 65536, 65536, 65536,
    ];
}

const INIT_ADDRESS: u8 = 0x01;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("new diagnostic mode not expected")]
    UnexpectedMode,
    #[error("response not expected for given command")]
    UnexpectedResponse,
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

    let mut client = Client { interface: port };

    if let Ok(_) = client.get_security_access() {
        println!("got security access");
    }

    client.switch_mode(DiagnosticMode::Programming, Some(38400))?;

    println!("in programming mode");

    client.interface.send(Message::GetDefaultTiming)?;
    dbg!(client.interface.next_response()?);
    client.interface.send(Message::GetTimingLimits)?;
    dbg!(client.interface.next_response()?);
    client.interface.send(Message::GetCurrentTiming)?;
    dbg!(client.interface.next_response()?);
    client.interface.send(Message::ChangeTimingParameters {
        p2min: 0,
        p2max: 40,
        p3min: 0,
        p3max: 20,
        p4min: 0,
    })?;
    dbg!(client.interface.next_response()?);

    client.get_security_access()?;

    println!("have pogramming security access");

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .open("memory_read.bin")?;

    let mut address = memory_layout::BASE_ADDRESS;
    for (n, size) in memory_layout::SECTORS.into_iter().enumerate() {
        println!("reading sector {} of {}", n, memory_layout::SECTORS.len());
        client.interface.send(Message::RequestDataTransfer {
            address,
            size,
            compression: CompressionFormat::Uncompressed,
            encryption: EncryptionFormat::Unencrypted,
            transfer_type: TransferType::Upload,
        })?;
        while let Ok(m) = client.interface.next_response() {
            if let Response::UploadConfirmation(_) = m {
                client.interface.send(Message::RequestData)?;
            } else if let Response::DataTransfer(d) = m {
                if !d.is_empty() {
                    file.write(&d)?;
                    client.interface.send(Message::RequestData)?;
                } else {
                    break;
                }
            } else {
                dbg!(m);
                panic!("unexpected");
            }
        }
        address += size;
        println!("  done.")
    }

    client.disconnect()?;

    println!("disconnected");

    Ok(())
}
