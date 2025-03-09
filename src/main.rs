use std::{fs::OpenOptions, io::Read, time::Duration};

use k_line::KLine;
use kwp2000::{
    Interface,
    client::Client,
    constants::{
        CompressionFormat, DiagnosticMode, EncryptionFormat, ServiceError, ServiceId,
        TimingParameter,
    },
    message::{Message, TransferType},
    response::{ProcessError, Response},
};

pub mod bcb;
pub mod k_line;
pub mod kwp2000;

pub mod memory_layout {
    pub const BASE_ADDRESS: u32 = 8388608;
    pub const SIZE: u32 = 1048576;

    pub const SECTORS: [u32; 19] = [
        16384, 8192, 8192, 32768, 65536, 65536, 65536, 65536, 65536, 65536, 65536, 65536, 65536,
        65536, 65536, 65536, 65536, 65536, 65536,
    ];

    pub const KEY: &[u8; 6] = b"GEHEIM";
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
    } else {
        println!("security failed");
    }

    println!("switching to programming mode");

    client.switch_mode(DiagnosticMode::Programming, Some(38400))?;

    println!("in programming mode");

    client.interface.send(Message::GetTimingLimits)?;

    if let Response::TimingParameters {
        kind: TimingParameter::Limits,
        p2min,
        p2max,
        p3min,
        p3max,
        p4min,
    } = client.interface.next_response()?
    {
        client.interface.send(Message::ChangeTimingParameters {
            p2min,
            p2max,
            p3min,
            p3max,
            p4min,
        })?;
        client.interface.next_response()?;
        println!("configured timings");
    }

    client.get_security_access()?;

    println!("have pogramming security access");

    let mut file = OpenOptions::new().read(true).open("write_test.bin")?;

    let mut address = memory_layout::BASE_ADDRESS;
    for (n, size) in memory_layout::SECTORS.into_iter().enumerate() {
        println!(
            "starting sector {} of {} with size {}",
            n + 1,
            memory_layout::SECTORS.len(),
            size
        );
        client.interface.send(Message::RequestDataTransfer {
            address,
            size,
            compression: CompressionFormat::Bosch,
            encryption: EncryptionFormat::Bosch,
            transfer_type: TransferType::Download,
        })?;
        let mut enc_index = 0;
        let mut max_len = 0;

        let mut data_buf = Vec::with_capacity(size as usize);
        file.read_exact(&mut data_buf)?;

        // uncompressed bytes sent so far
        let mut sent_bytes = 0;
        while let Ok(m) = client.interface.next_response() {
            let send = if let Response::DownloadConfirmation(max) = m {
                max_len = max as usize;
                Some(true)
            } else if let Response::ReadyForMoreData = m {
                Some(false)
            } else if let Response::Error(ProcessError {
                error: ServiceError::RoutineNotComplete,
                service: ServiceId::RequestDownload,
            }) = m
            {
                continue;
            } else {
                dbg!(m);
                client.disconnect()?;
                panic!("unexpected");
            };

            if let Some(first) = send {
                if sent_bytes >= data_buf.len() {
                    break;
                }
                let (sent, transfer_block) = bcb::encrypt_and_compress(
                    max_len,
                    &mut data_buf[sent_bytes..],
                    &mut enc_index,
                    memory_layout::KEY,
                    first,
                )?;

                client.interface.send(Message::SendData(transfer_block))?;

                sent_bytes += sent;
            }
        }
        address += size;
        println!("  done.")
    }

    client.disconnect()?;

    println!("disconnected");

    Ok(())
}
