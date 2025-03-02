use std::time::Duration;

use k_line::KLine;
use kwp2000::{
    constants::{DiagnosticMode, Service, ServiceError, ServiceId, ServiceResponse},
    message::RawMessage,
};

pub mod k_line;
pub mod kwp2000;

const INIT_ADDRESS: u8 = 0x01;
const COM_ADDRESS: u8 = 0x10;

const TESTER_ADDRESS: u8 = 0xF1;

fn main() -> Result<(), serialport::Error> {
    let mut port = serialport::new("/dev/ttyUSB0", 10400)
        .timeout(Duration::from_millis(300))
        .flow_control(serialport::FlowControl::None)
        .open()
        .unwrap();

    port.init_kwp2000(INIT_ADDRESS)?;

    println!("init done");

    port.write_message(RawMessage::new_query_physical(
        ServiceId::StartCommunication,
        COM_ADDRESS,
        TESTER_ADDRESS,
        Vec::new(),
    ))?;

    let address = 0x380da8;
    let mut defined = false;

    while let Ok(m) = RawMessage::from_bytes(&mut port) {
        match m.service {
            Service::Response(ServiceResponse::StartCommunication) => {
                port.write_message(RawMessage::new_query_none(
                    ServiceId::StartDiagnosticSession,
                    vec![DiagnosticMode::Diagnostics as u8],
                ))?;
            }
            Service::Response(ServiceResponse::StartDiagnosticSession) => {
                if m.data[0] == DiagnosticMode::Programming as u8 {
                    port.write_message(RawMessage::new_query_none(
                        ServiceId::ReadMemoryByAddress,
                        vec![0x38, 0x22, 0x62],
                    ))?;
                } else {
                    port.write_message(RawMessage::new_query_none(
                        ServiceId::DynamicallyDefineLocalIdentifier,
                        vec![0xf0, 0x04],
                    ))?;
                }
            }
            Service::Response(ServiceResponse::DynamicallyDefineLocalIdentifier) => {
                if defined {
                    port.write_message(RawMessage::new_query_physical(
                        ServiceId::SecurityAccess,
                        COM_ADDRESS,
                        TESTER_ADDRESS,
                        vec![0x01],
                    ))?;
                } else {
                    port.write_message(ddli(address))?;
                    defined = true;
                }
            }
            Service::Response(ServiceResponse::NegativeResponse) => {
                let regarding = Service::try_from(m.data[0]).unwrap();
                let reason = ServiceError::from_repr(m.data[1]).unwrap();
                if regarding == Service::Query(ServiceId::SecurityAccess) {
                    port.write_message(writeli())?;
                } else {
                    println!("Error regarding: {:?} because {:?}.", regarding, reason);
                }
            }
            Service::Response(ServiceResponse::ReadECUIdentification) => {
                println!("ECUID response: {}", String::from_utf8_lossy(&m.data));
            }
            Service::Response(ServiceResponse::WriteDataByLocalIdentifier) => {
                port.write_message(RawMessage::new_query_physical(
                    ServiceId::SecurityAccess,
                    COM_ADDRESS,
                    TESTER_ADDRESS,
                    vec![0x01],
                ))?;
                println!("wrote");
            }
            Service::Response(ServiceResponse::SecurityAccess) => {
                if m.data[0] == 0x01 {
                    let mut key = kwp2000::security_key_from_seed(&m.data[1..5]);
                    key.insert(0, 0x02);
                    port.write_message(RawMessage::new_query_none(ServiceId::SecurityAccess, key))?;
                    println!("sent security response");
                } else if m.data[0] == 0x02 {
                    println!("Got security access");

                    // port.write_message(RawMessage::new_query_none(
                    //     ServiceId::StartDiagnosticSession,
                    //     vec![DiagnosticMode::Programming as u8],
                    // ))?;
                }
            }
            _ => {}
        }
    }

    Ok(())
}
fn ddli(address: u32) -> RawMessage {
    let bytes = address.to_be_bytes();
    let mut data = vec![0xF0, 0x03, 0x01, 0x02];
    for i in 1..4 {
        data.push(bytes[i]);
    }
    RawMessage::new_query_none(ServiceId::DynamicallyDefineLocalIdentifier, data)
}

// fn readli() -> RawMessage {
//     RawMessage::new_query_none(ServiceId::ReadDataByLocalIdentifier, vec![0xF0, 0x01, 0x01])
// }

fn writeli() -> RawMessage {
    RawMessage::new_query_none(ServiceId::WriteDataByLocalIdentifier, vec![0xF0, 0, 0])
}
