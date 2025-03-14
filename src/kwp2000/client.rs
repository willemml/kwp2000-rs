use std::{fmt::Debug, io::ErrorKind};

use crate::{
    Error,
    kwp2000::{
        constants::{ReadMode, ServiceError, ServiceId},
        response::ProcessError,
        security_key_from_seed,
    },
};

use super::{
    Interface,
    constants::{CompressionFormat, DiagnosticMode, EncryptionFormat, TimingParameter},
    message::{Message, TransferType},
    response::Response,
};

pub trait DebugInterface: Interface + Debug {}

impl DebugInterface for serialport::TTYPort {}

#[derive(Debug)]
pub struct Client {
    pub interface: Box<dyn DebugInterface>,
}

macro_rules! message_chain {
    {$client:ident => {
        $($message:expr => {
            $($response:pat => $respond:block)*
        })*
    }} => {
        $(
            $client.interface.send($message)?;

            match $client.interface.next_response()? {
                $($response => $respond,)*
                r => return Err(Error::UnexpectedResponse(r)),
            }
        )*
    };
}

impl Client {
    pub fn dd_write_address(&mut self, address: u32, data: Vec<u8>) -> Result<(), Error> {
        assert!(data.len() <= 253);
        message_chain! {self => {
            Message::ClearLocalIdentifier(0xF0) => {
                Response::LocalIdentifierDefined(0xF0) => {}
            }
            Message::DefineLocalIdentifierAddress(0xF0, data.len() as u8, address) => {
                Response::LocalIdentifierDefined(0xF0) => {}
            }
            Message::WriteLocalIdentifier(0xF0, data) => {
                Response::LocalIdentifierWritten(0xF0) => {return Ok(())}
            }
        }}
    }
    pub fn dd_read_address(&mut self, address: u32, length: u8) -> Result<Vec<u8>, Error> {
        message_chain! {self => {
            Message::ClearLocalIdentifier(0xF0) => {
                Response::LocalIdentifierDefined(0xF0) => {}
            }
            Message::DefineLocalIdentifierAddress(0xF0, length, address) => {
                Response::LocalIdentifierDefined(0xF0) => {}
            }
            Message::ReadLocalIdentifier(0xF0, ReadMode::Single, 1) => {
                Response::LocalIdentifierRead(_, data) => {return Ok(data)}
            }
        }}
    }
    pub fn clear_security_wait(&mut self) -> Result<(), Error> {
        self.dd_write_address(0x380da8, vec![0, 0])
    }
    pub fn read_data<W: std::io::Write>(
        &mut self,
        address: u32,
        size: u32,
        destination: &mut W,
    ) -> Result<usize, Error> {
        self.interface.send(Message::RequestDataTransfer {
            address,
            size,
            compression: CompressionFormat::Uncompressed,
            encryption: EncryptionFormat::Unencrypted,
            transfer_type: TransferType::Upload,
        })?;
        let mut written = 0;
        while let Ok(m) = self.interface.next_response() {
            if let Response::UploadConfirmation(_) = m {
                self.interface.send(Message::RequestData)?;
            } else if let Response::DataTransfer(d) = m {
                if !d.is_empty() {
                    written += d.len();
                    destination.write(&d)?;
                    self.interface.send(Message::RequestData)?;
                } else {
                    break;
                }
            } else {
                return Err(Error::UnexpectedResponse(m));
            }
        }
        return Ok(written);
    }
    pub fn write_data_bosch(&mut self, address: u32, data: &[u8], key: &[u8]) -> Result<(), Error> {
        self.interface.send(Message::RequestDataTransfer {
            address,
            size: data.len() as u32,
            compression: CompressionFormat::Bosch,
            encryption: EncryptionFormat::Bosch,
            transfer_type: TransferType::Download,
        })?;
        let mut enc_index = 0;
        let mut max_len = 0;

        // uncompressed bytes sent so far
        let mut sent_bytes = 0;

        let mut response = self.interface.next_response();
        while let Ok(m) = response {
            response = self.interface.next_response();
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
                return Err(Error::UnexpectedResponse(m));
            };

            if let Some(first) = send {
                if sent_bytes >= data.len() {
                    break;
                }
                let (sent, transfer_block) = crate::bcb::encrypt_and_compress(
                    max_len,
                    &data[sent_bytes..],
                    &mut enc_index,
                    key,
                    first,
                )?;

                self.interface.send(Message::SendData(transfer_block))?;

                sent_bytes += sent;
            }
        }

        if let Err(e) = response {
            match e {
                Error::Io(error) if error.kind() == ErrorKind::TimedOut => {}
                Error::SerialPort(error)
                    if error.kind() == serialport::ErrorKind::Io(ErrorKind::TimedOut) => {}
                _ => return Err(e),
            }
        }
        Ok(())
    }
    pub fn use_fastest_timing(&mut self) -> Result<(), Error> {
        self.interface.send(Message::GetTimingLimits)?;
        let response = self.interface.next_response()?;
        Err(Error::UnexpectedResponse(
            if let Response::TimingParameters {
                kind: TimingParameter::Limits,
                p2min,
                p2max,
                p3min,
                p3max,
                p4min,
            } = response
            {
                self.interface.send(Message::ChangeTimingParameters {
                    p2min,
                    p2max,
                    p3min,
                    p3max,
                    p4min,
                })?;
                let response = self.interface.next_response()?;
                if let Response::TimingSet = response {
                    return Ok(());
                } else {
                    response
                }
            } else {
                response
            },
        ))
    }
    pub fn new(interface: Box<dyn DebugInterface>) -> Client {
        Client { interface }
    }
    pub fn disconnect(mut self) -> Result<(), Error> {
        message_chain! {self => {
            Message::StopDiagnosticSession => {
                Response::DiagnosticSessionStopped => {}
                Response::Error(_) => {}
            }
            Message::StopCommunication => {
                Response::CommunicationStopped => {}
            }
        }}

        Ok(())
    }

    fn switch_mode(
        &mut self,
        new_mode: DiagnosticMode,
        baud_rate: Option<u32>,
    ) -> Result<(), Error> {
        message_chain! {self => {
            Message::StartDiagnosticSession(new_mode, baud_rate) => {
                Response::StartedDiagnosticMode(mode, new_baud) => {
                    if let Some(baud) = new_baud {
                        self.interface.switch_baud(baud)?;
                    }
                    if mode == new_mode {
                        Ok(())
                    } else {
                        Err(Error::UnexpectedMode)
                    }
                }
            }
        }}
    }
    pub fn programming_mode(&mut self, baud_rate: Option<u32>) -> Result<(), Error> {
        self.switch_mode(DiagnosticMode::Programming, baud_rate)
    }

    pub fn developer_mode(&mut self, baud_rate: Option<u32>) -> Result<(), Error> {
        self.switch_mode(DiagnosticMode::Programming, baud_rate)
    }

    pub fn diagnostic_mode(&mut self) -> Result<(), Error> {
        self.switch_mode(DiagnosticMode::Diagnostics, None)
    }
    pub fn get_security_access(&mut self) -> Result<(), Error> {
        let seed_arr;
        message_chain! {self => {
            Message::RequestSecuritySeed => {
                Response::SecurityAccessSeed(_, seed) => {
                    seed_arr = seed.to_vec().try_into().unwrap();
                }
                Response::SecurityAccessGranted(_) => {
                    return Ok(());
                }
            }
            Message::SendSecurityKey(security_key_from_seed(seed_arr)) => {
                Response::SecurityAccessGranted(_) => {
                    return Ok(());
                }
                Response::Error(ProcessError {
                    error: ServiceError::TooManyAttempts | ServiceError::RequestingTooFast,
                    service: ServiceId::SecurityAccess,
                }) => {
                    return self.get_security_access();
                }
            }
        }}
    }
}
