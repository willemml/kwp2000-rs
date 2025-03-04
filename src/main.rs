use std::time::Duration;

use k_line::KLine;
use kwp2000::{client::Client, constants::DiagnosticMode};

pub mod k_line;
pub mod kwp2000;

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
        .timeout(Duration::from_millis(1000))
        .flow_control(serialport::FlowControl::None)
        .open()
        .unwrap();

    port.init_kwp2000(INIT_ADDRESS)?;

    println!("init done");

    let mut client = Client { interface: port };

    client.get_security_access()?;

    println!("got security access");

    client.switch_mode(DiagnosticMode::Programming)?;

    println!("in programming mode");

    client.disconnect()?;

    println!("disconnected");

    Ok(())
}
