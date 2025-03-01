use std::{io::Read, thread::sleep, time::Duration};

use kwp::{DiagnosticMode, RawMessage, Service, ServiceId};
use serialport::SerialPort;

pub mod kwp;

const INIT_ADDRESS: u8 = 0x01;
const COM_ADDRESS: u8 = 0x10;

const TESTER_ADDRESS: u8 = 0xF1;

trait KLine {
    type Error;

    fn send_init_5baud(&mut self, address: u8) -> Result<(), Self::Error> {
        // Idle for 300ms before sending anything.
        self.set_low()?;
        sleep(Duration::from_millis(300));

        // Send high bit to start transfer.
        self.set_high()?;
        sleep(Duration::from_millis(200));

        // Send target address at 5 baud
        self.bitbang(5, address)?;
        Ok(())
    }

    fn init_kwp2000(&mut self, address: u8) -> Result<(), Self::Error> {
        self.send_init_5baud(address)?;

        self.wait_for_byte(0x55)?;

        self.wait_for_byte(0x8F)?;

        // Wait a bit before sending complement of key byte 2
        sleep(Duration::from_millis(25));
        self.write_byte(0xFF - 0x8F)?;

        self.wait_for_byte(0xFF - address)?;

        Ok(())
    }

    fn bitbang(&mut self, baud: u8, byte: u8) -> Result<(), Self::Error> {
        let delay = Duration::from_millis(1_000 / baud as u64);

        for state in (0..8).map(|n| ((1 << n) & byte) == 0) {
            if state {
                self.set_high()?;
            } else {
                self.set_low()?;
            }
            sleep(delay);
        }

        // Set low to allow incoming data
        self.set_low()?;

        Ok(())
    }

    fn wait_for_byte(&mut self, byte: u8) -> Result<(), Self::Error> {
        while self.read_byte()? != byte {
            continue;
        }
        Ok(())
    }

    fn write_message(&mut self, messsage: RawMessage) -> Result<(), Self::Error>;

    fn write_byte(&mut self, byte: u8) -> Result<(), Self::Error>;
    fn read_byte(&mut self) -> Result<u8, Self::Error>;

    fn set_high(&mut self) -> Result<(), Self::Error>;
    fn set_low(&mut self) -> Result<(), Self::Error>;
}

impl KLine for Box<dyn SerialPort> {
    type Error = serialport::Error;

    fn read_byte(&mut self) -> Result<u8, Self::Error> {
        let mut buf = [0u8];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn write_message(&mut self, message: RawMessage) -> Result<(), Self::Error> {
        self.write_all(&message.to_bytes())?;
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set_break()
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.clear_break()
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), Self::Error> {
        self.write_all(&[byte])?;
        Ok(())
    }
}

fn main() -> Result<(), serialport::Error> {
    let mut port = serialport::new("/dev/ttyUSB0", 10400)
        .timeout(Duration::from_millis(300))
        .flow_control(serialport::FlowControl::None)
        .open()
        .unwrap();

    port.init_kwp2000(INIT_ADDRESS)?;

    println!("init done");

    port.write_message(RawMessage::new_query_physical(
        kwp::ServiceId::StartCommunication,
        COM_ADDRESS,
        TESTER_ADDRESS,
        Vec::new(),
    ))?;

    while let Ok(m) = RawMessage::from_bytes(&mut port) {
        match m.service {
            Service::Response(kwp::ServiceResponse::StartCommunication) => {
                port.write_message(RawMessage::new_query_none(
                    ServiceId::StartDiagnosticSession,
                    vec![DiagnosticMode::Diagnostics as u8],
                ))?;
            }
            Service::Response(kwp::ServiceResponse::StartDiagnosticSession) => {
                port.write_message(RawMessage::new_query_none(
                    ServiceId::ReadECUIdentification,
                    vec![0x80],
                ))?;
            }
            Service::Response(kwp::ServiceResponse::NegativeResponse) => {
                let regarding = Service::try_from(m.data[0]).unwrap();
                let reason = kwp::ServiceError::from_repr(m.data[1]).unwrap();
                println!("Error regarding: {:?} because {:?}.", regarding, reason);
            }
            Service::Response(kwp::ServiceResponse::ReadECUIdentification) => {
                println!("ECUID response: {}", String::from_utf8_lossy(&m.data));
            }
            Service::Query(q) => {
                println!("Got echo for {:?}", q);
            }
            _ => {
                dbg!(m);
            }
        }
    }

    Ok(())
}
