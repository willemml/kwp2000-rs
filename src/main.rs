use std::time::Duration;

use k_line::KLine;
use kwp2000::{
    Interface,
    constants::{DiagnosticMode, ServiceError},
    message::Message,
    raw_message::RawMessage,
    response::{ProcessError, Response},
};

pub mod k_line;
pub mod kwp2000;

const INIT_ADDRESS: u8 = 0x01;

#[derive(Debug, thiserror::Error)]
pub enum Error {
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
        .timeout(Duration::from_millis(1000))
        .flow_control(serialport::FlowControl::None)
        .open()
        .unwrap();

    port.init_kwp2000(INIT_ADDRESS)?;

    println!("init done");

    port.send(Message::StartDiagnosticSession(
        DiagnosticMode::Programming,
        Some(38400),
    ))?;

    let mut defined = false;

    loop {
        match kwp2000::response::from_raw(RawMessage::from_bytes(&mut port)?)? {
            Response::StartedDiagnosticMode(diagnostic_mode) => {
                println!("Entered diag mode: {:?}", diagnostic_mode);
                port.send(Message::RequestSecuritySeed)?;
                port.send(Message::RequestSecuritySeed)?;
            }
            Response::SecurityAccessSeed(_level, seed) => {
                let key = kwp2000::security_key_from_seed(seed.try_into().unwrap());
                port.send(Message::SendSecurityKey(key))?;
                println!("sent security response");
            }
            Response::SecurityAccessGranted(_level) => {
                println!("got security access");
                port.send(Message::StartDiagnosticSession(
                    DiagnosticMode::Programming,
                    Some(10400),
                ))?;
            }
            Response::LocalIdentifierDefined(_) => {
                if defined {
                    // Set the security timeout to 0.
                    port.send(Message::WriteLocalIdentifier(0xF0, vec![0, 0]))?;
                } else {
                    // Define the local identifier to the location of the security timout
                    // bytes in RAM.
                    port.send(Message::DefineLocalIdentifierAddress(0xF0, 2, 0x380da8))?;
                    defined = true;
                }
            }
            Response::LocalIdentifierWritten(_) => port.send(Message::RequestSecuritySeed)?,
            Response::LocalIdentifierRead(id, bytes) => {
                println!("local 0x{:02x}: {:02x?}", id, bytes)
            }
            Response::StillProcessing(service_id) => {
                println!("still processing {:?} request", service_id);
            }
            Response::Error(ProcessError { error, service }) => match error {
                ServiceError::SecurityAccessRequired => {
                    port.send(Message::RequestSecuritySeed)?;
                }
                ServiceError::RequestingTooFast => {
                    port.send(Message::ClearLocalIdentifier(0xF0))?;
                }
                e => println!("Error: {:?} returned {:?}", service, e),
            },
            _ => {}
        }
    }
}
