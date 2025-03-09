use std::{fmt::Debug, io::ErrorKind, marker::PhantomData};

use crate::{
    Error,
    kwp2000::{
        constants::{ServiceError, ServiceId},
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

#[derive(Debug)]
pub struct ProgrammingMode;
#[derive(Debug)]
pub struct DeveloperMode;
#[derive(Debug)]
pub struct DiagnosticsMode;
#[derive(Debug)]
pub struct OBDMode;
#[derive(Debug)]
pub struct UnknownMode;
#[derive(Debug)]
pub struct Disconnected;

#[derive(Debug)]
pub struct WithSecurity;
#[derive(Debug)]
pub struct WithoutSecurity;

impl<M, S> From<(Client<M, S>, Error)> for Error {
    fn from(value: (Client<M, S>, Error)) -> Self {
        value.1
    }
}

macro_rules! FnTrait {
    ($($mode:ident),* => $name:ident) => {
        pub trait $name {}
        $(
        impl $name for $mode {}
        )*
    };
}

FnTrait!(ProgrammingMode, DeveloperMode => WriteDataBosch);
FnTrait!(ProgrammingMode, DeveloperMode => AccessTiming);

pub trait DebugInterface: Interface + Debug {}

impl DebugInterface for serialport::TTYPort {}

#[derive(Debug)]
pub struct Client<M, S> {
    pub interface: Box<dyn DebugInterface>,

    _mode: PhantomData<M>,
    _security: PhantomData<S>,
}

macro_rules! client_error {
    ($client:ident, $code:expr) => {
        match $code {
            Ok(r) => Ok(r),
            Err(e) => return Err(($client, e)),
        }
    };
}

macro_rules! message_chain {
    {$client:ident => {
        $($message:expr => {
            $($response:pat => $respond:block)*
        })*
    }} => {
        $(
            client_error!($client, $client.interface.send($message))?;

            match client_error!($client, $client.interface.next_response())? {
                $($response => $respond,)*
                r => return Err(($client,Error::UnexpectedResponse(r))),
            }
        )*
    };
}

impl<M, S> Client<M, S> {
    pub fn clear_security_wait(mut self) -> Result<Self, (Self, Error)> {
        message_chain! {self => {
            Message::ClearLocalIdentifier(0xF0) => {
                Response::LocalIdentifierDefined(0xF0) => {}
            }
            Message::DefineLocalIdentifierAddress(0xF0, 2, 0x380da8) => {
                Response::LocalIdentifierDefined(0xF0) => {}
            }
            Message::WriteLocalIdentifier(0xF0, vec![0,0]) => {
                Response::LocalIdentifierWritten(0xF0) => {return Ok(self)}
            }
        }}
    }
}

impl Client<ProgrammingMode, WithSecurity> {
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
}

impl<M: WriteDataBosch> Client<M, WithSecurity> {
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
}

impl<M: AccessTiming> Client<M, WithoutSecurity> {
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
}
impl Client<UnknownMode, WithoutSecurity> {
    pub fn new(interface: Box<dyn DebugInterface>) -> Client<UnknownMode, WithoutSecurity> {
        Client {
            interface,
            _mode: PhantomData,
            _security: PhantomData,
        }
    }
}

impl<M, S> Client<M, S> {
    pub fn disconnect(mut self) -> Result<Client<Disconnected, WithoutSecurity>, (Self, Error)> {
        message_chain! {self => {
            Message::StopDiagnosticSession => {
                Response::DiagnosticSessionStopped => {}
                Response::Error(_) => {}
            }
            Message::StopCommunication => {
                Response::CommunicationStopped => {}
            }
        }}

        Ok(self.with_state())
    }

    fn with_state<NM, NS>(self) -> Client<NM, NS> {
        Client {
            interface: self.interface,
            _mode: PhantomData,
            _security: PhantomData,
        }
    }

    fn switch_mode<NM>(
        mut self,
        new_mode: DiagnosticMode,
        baud_rate: Option<u32>,
    ) -> Result<Client<NM, WithoutSecurity>, (Self, Error)> {
        message_chain! {self => {
            Message::StartDiagnosticSession(new_mode, baud_rate) => {
                Response::StartedDiagnosticMode(mode, new_baud) => {
                    if let Some(baud) = new_baud {
                        client_error!(self, self.interface.switch_baud(baud))?;
                    }
                    if mode == new_mode {
                        Ok(self.with_state())
                    } else {
                        Err((self, Error::UnexpectedMode))
                    }
                }
            }
        }}
    }
    pub fn programming_mode(
        self,
        baud_rate: Option<u32>,
    ) -> Result<Client<ProgrammingMode, WithoutSecurity>, (Self, Error)> {
        self.switch_mode(DiagnosticMode::Programming, baud_rate)
    }
}
impl<M, S> Client<M, S> {
    pub fn get_security_access(mut self) -> Result<Client<M, WithSecurity>, (Self, Error)> {
        let seed_arr;
        message_chain! {self => {
            Message::RequestSecuritySeed => {
                Response::SecurityAccessSeed(_, seed) => {
                    seed_arr = seed.to_vec().try_into().unwrap();
                }
                Response::SecurityAccessGranted(_) => {
                    return Ok(self.with_state());
                }
            }
            Message::SendSecurityKey(security_key_from_seed(seed_arr)) => {
                Response::SecurityAccessGranted(_) => {
                    return Ok(self.with_state());
                }
                Response::Error(ProcessError {
                    error: ServiceError::TooManyAttempts | ServiceError::RequestingTooFast,
                    service: ServiceId::SecurityAccess,
                }) => {
                    return Err((self, Error::SecurityTimout))
                }
            }
        }}
    }
    pub fn security_access_timeout_bypass(self) -> Result<Client<M, WithSecurity>, (Self, Error)> {
        match self.get_security_access() {
            Ok(new) => Ok(new),
            Err((c, e)) => match e {
                Error::SecurityTimout => Ok(c.clear_security_wait()?.get_security_access()?),
                e => Err((c, e)),
            },
        }
    }
}
