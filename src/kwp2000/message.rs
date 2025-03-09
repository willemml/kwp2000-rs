use super::raw_message::RawMessage;
use super::{baud_rate_to_byte, constants::*};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferType {
    Download,
    Upload,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// optional baudrate
    StartDiagnosticSession(DiagnosticMode, Option<u32>),
    StopCommunication,
    RequestSecuritySeed,
    ClearLocalIdentifier(u8),
    /// identifier, mode, maximum response count
    ReadLocalIdentifier(u8, ReadMode, u8),
    WriteLocalIdentifier(u8, Vec<u8>),
    /// identifier, length, address
    DefineLocalIdentifierAddress(u8, u8, u32),
    SendSecurityKey(u32),
    /// whether or not server should respond
    TesterPresent(bool),
    StopDiagnosticSession,
    ReadMemoryByAddress {
        address: u32,
        size: u8,
        mode: Option<ReadMode>,
        /// requires that `mode` be set
        max_response_count: Option<u8>,
    },
    RequestDataTransfer {
        transfer_type: TransferType,
        address: u32,
        size: u32,
        encryption: EncryptionFormat,
        compression: CompressionFormat,
    },
    RequestData,
    GetCurrentTiming,
    GetDefaultTiming,
    GetTimingLimits,
    ChangeTimingParameters {
        /// minimum time between tester message and ecu response
        /// resolution: 0.5 ms/bit
        p2min: u8,
        /// maximum time between tester message and ecu response
        /// resolution: 25 ms/bit
        p2max: u8,
        /// minimum time betwwen ecu response and tester request
        /// resolution: 0.5ms/bit
        p3min: u8,
        /// maximum time between ecu response and tester request
        /// resolution: 250ms/bit
        p3max: u8,
        /// maximum interbyte time from tester (from ecu is P1, not configurable)
        /// resolution: 0.5ms/bit
        p4min: u8,
    },
}

impl Message {
    pub fn raw(self) -> RawMessage {
        let service;
        let mut data: Vec<u8> = vec![];
        match self {
            Self::ChangeTimingParameters {
                p2min,
                p2max,
                p3min,
                p3max,
                p4min,
            } => {
                service = ServiceId::AccessTimingParameter;
                data.push(TimingParameter::Set as u8);
                for p in [p2min, p2max, p3min, p3max, p4min] {
                    data.push(p);
                }
            }
            Self::GetCurrentTiming => {
                service = ServiceId::AccessTimingParameter;
                data.push(TimingParameter::Read as u8);
            }
            Self::GetTimingLimits => {
                service = ServiceId::AccessTimingParameter;
                data.push(TimingParameter::Limits as u8);
            }
            Self::GetDefaultTiming => {
                service = ServiceId::AccessTimingParameter;
                data.push(TimingParameter::Defaults as u8);
            }
            Message::RequestData => {
                service = ServiceId::TransferData;
            }
            Message::RequestDataTransfer {
                transfer_type,
                address,
                size,
                encryption,
                compression,
            } => {
                service = match transfer_type {
                    TransferType::Download => ServiceId::RequestDownload,
                    TransferType::Upload => ServiceId::RequestUpload,
                };

                let bytes = address.to_be_bytes();
                for b in &bytes[1..4] {
                    data.push(*b);
                }

                data.push(data_format_byte(compression, encryption));

                let bytes = size.to_be_bytes();
                for b in &bytes[1..4] {
                    data.push(*b);
                }
            }
            Message::ReadMemoryByAddress {
                address,
                size,
                mode,
                max_response_count,
            } => {
                service = ServiceId::ReadMemoryByAddress;
                let bytes = address.to_be_bytes();
                for b in &bytes[1..4] {
                    data.push(*b);
                }
                data.push(size);
                mode.map(|mode| data.push(mode as u8));
                max_response_count.map(|m| data.push(m));
            }
            Message::StopDiagnosticSession => {
                service = ServiceId::StopDiagnosticSession;
            }
            Message::StartDiagnosticSession(diagnostic_mode, baud) => {
                service = ServiceId::StartDiagnosticSession;
                data.push(diagnostic_mode as u8);
                baud.map(|b| data.push(baud_rate_to_byte(b)));
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
            Message::StopCommunication => service = ServiceId::StopCommunication,
        }
        RawMessage::new_query(service, data)
    }
}
