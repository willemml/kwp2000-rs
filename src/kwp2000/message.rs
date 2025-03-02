use super::raw_message::RawMessage;
use super::{calculate_baud_rate_byte, constants::*};

pub enum Message {
    /// optional baudrate
    StartDiagnosticSession(DiagnosticMode, Option<u32>),
    RequestSecuritySeed,
    ClearLocalIdentifier(u8),
    /// identifier, mode, maximum response count
    ReadLocalIdentifier(u8, LocalIdentifierReadMode, u8),
    WriteLocalIdentifier(u8, Vec<u8>),
    /// identifier, length, address
    DefineLocalIdentifierAddress(u8, u8, u32),
    SendSecurityKey(u32),
    /// whether or not server should respond
    TesterPresent(bool),
}

impl Message {
    pub fn raw(self) -> RawMessage {
        let service;
        let mut data: Vec<u8> = vec![];
        match self {
            Message::StartDiagnosticSession(diagnostic_mode, baud) => {
                service = ServiceId::StartDiagnosticSession;
                data.push(diagnostic_mode as u8);
                baud.map(|b| data.push(calculate_baud_rate_byte(b)));
            }
            Message::RequestSecuritySeed => {
                service = ServiceId::SecurityAccess;
                // TODO: allow different access levels/modes
                data.push(0x01);
            }
            Message::ClearLocalIdentifier(id) => {
                service = ServiceId::DynamicallyDefineLocalIdentifier;
                data.push(id);
                data.push(DynamicDefinitionMode::ClearDynamicallyDefinedLocalIdentifier as u8);
            }
            Message::ReadLocalIdentifier(id, mode, count) => {
                service = ServiceId::ReadDataByLocalIdentifier;
                data.push(id);
                data.push(mode as u8);
                data.push(count);
            }
            Message::WriteLocalIdentifier(id, mut items) => {
                service = ServiceId::WriteDataByLocalIdentifier;
                data.push(id);
                data.append(&mut items);
            }
            Message::DefineLocalIdentifierAddress(id, size, address) => {
                service = ServiceId::DynamicallyDefineLocalIdentifier;
                data.push(id);
                data.push(DynamicDefinitionMode::DefineByMemoryAddress as u8);
                // TODO: allow different positions in definition
                data.push(0x01);
                data.push(size);
                for b in address.to_be_bytes().into_iter().skip(1) {
                    data.push(b);
                }
            }
            Message::SendSecurityKey(key) => {
                service = ServiceId::SecurityAccess;
                data.push(0x02);
                for n in key.to_be_bytes() {
                    data.push(n);
                }
            }
            Message::TesterPresent(respond) => {
                service = ServiceId::TesterPresent;
                data.push(if respond { 0x01 } else { 0x02 });
            }
        }
        RawMessage::new_query(service, data)
    }
}
