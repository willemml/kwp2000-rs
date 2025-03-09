use super::constants::*;
use super::raw_message::RawMessage;
use crate::Error;
use crate::kwp2000::baud_rate_from_byte;

pub fn from_raw(mut message: RawMessage) -> Result<Response, Error> {
    Ok(match &message.service {
        Service::Query(_) => Response::Echo(message),
        Service::Response(service_response) => match service_response {
            ServiceResponse::AccessTimingParameter => {
                let kind = TimingParameter::from_repr(message.data[0]).unwrap();
                if kind == TimingParameter::Defaults {
                    Response::TimingRestoredToDefault
                } else if kind == TimingParameter::Set {
                    Response::TimingSet
                } else {
                    Response::TimingParameters {
                        kind,
                        p2min: message.data[1],
                        p2max: message.data[2],
                        p3min: message.data[3],
                        p3max: message.data[4],
                        p4min: message.data[5],
                    }
                }
            }
            ServiceResponse::ReadMemoryByAddress => {
                let mut bytes = [0u8; 4];
                for i in 3..0 {
                    bytes[i] = message.data.pop().ok_or(Error::NotEnoughData)?;
                }
                Response::MemoryAddressRead(u32::from_be_bytes(bytes), message.data)
            }
            ServiceResponse::NegativeResponse => {
                let error = ProcessError::from_bytes(&message.data)?;
                if error.error == ServiceError::ResponsePending {
                    Response::StillProcessing(error.service)
                } else {
                    Response::Error(error)
                }
            }
            ServiceResponse::StartDiagnosticSession => Response::StartedDiagnosticMode(
                DiagnosticMode::from_repr(message.data[0]).ok_or(Error::UnexpectedValue)?,
                message.data.get(1).map(|x| baud_rate_from_byte(*x)),
            ),
            ServiceResponse::ReadDataByLocalIdentifier => {
                Response::LocalIdentifierRead(message.data[0], message.data.split_off(1))
            }
            ServiceResponse::TesterPresent => Response::TesterPresent,
            ServiceResponse::SecurityAccess => {
                if message.data.len() == 2
                    || message.data[1..].iter().max().map_or(false, |m| m == &0)
                {
                    Response::SecurityAccessGranted(
                        SecurityLevel::from_repr(message.data[0]).ok_or(Error::UnexpectedValue)?,
                    )
                } else {
                    let seed_level =
                        SecurityLevel::from_repr(message.data[0]).ok_or(Error::UnexpectedValue)?;

                    Response::SecurityAccessSeed(seed_level, message.data.split_off(1))
                }
            }
            ServiceResponse::DynamicallyDefineLocalIdentifier => {
                Response::LocalIdentifierDefined(message.data[0])
            }
            ServiceResponse::WriteDataByLocalIdentifier => {
                Response::LocalIdentifierWritten(message.data[0])
            }
            ServiceResponse::StopCommunication => Response::CommunicationStopped,
            ServiceResponse::StopDiagnosticSession => Response::DiagnosticSessionStopped,
            ServiceResponse::RequestUpload => Response::UploadConfirmation(message.data[0]),
            ServiceResponse::RequestDownload => Response::DownloadConfirmation(message.data[0]),
            ServiceResponse::TransferData => {
                if message.data.is_empty() {
                    Response::ReadyForMoreData
                } else {
                    Response::DataTransfer(message.data)
                }
            }
            _ => {
                dbg!(message);
                return Err(Error::NotImplemented);
            }
        },
    })
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessError {
    pub error: ServiceError,
    pub service: ServiceId,
}

impl ProcessError {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let service = ServiceId::from_repr(bytes[0]).ok_or(Error::InvalidService)?;
        let error = ServiceError::from_repr(bytes[1]).ok_or(Error::InvalidServiceError)?;
        Ok(Self { error, service })
    }
}

#[derive(Debug, Clone)]
pub enum Response {
    MemoryAddressRead(u32, Vec<u8>),
    DiagnosticSessionStopped,
    CommunicationStopped,
    /// Query type messages from the server are all considered echoes
    Echo(RawMessage),
    Error(ProcessError),
    LocalIdentifierDefined(u8),
    LocalIdentifierRead(u8, Vec<u8>),
    LocalIdentifierWritten(u8),
    /// If the returned SecurityKeyLevel is greater than 1, there are higher
    /// levels of access available.
    SecurityAccessGranted(SecurityLevel),
    /// If the returned SecuritySeedLevel is greater than 1, there are higher
    /// levels of access available. If received a security access seed with
    /// a number greater than 1, send the key with one access level higher
    /// than originally requested.
    SecurityAccessSeed(SecurityLevel, Vec<u8>),
    /// mode, baud rate
    StartedDiagnosticMode(DiagnosticMode, Option<u32>),
    StillProcessing(ServiceId),
    TesterPresent,
    /// Data response for request upload
    DataTransfer(Vec<u8>),
    ReadyForMoreData,
    /// Maximum block length returned
    UploadConfirmation(u8),
    /// Maximum block length to send
    DownloadConfirmation(u8),
    /// See the Message enum for details
    TimingParameters {
        kind: TimingParameter,
        p2min: u8,
        p2max: u8,
        p3min: u8,
        p3max: u8,
        p4min: u8,
    },
    TimingRestoredToDefault,
    TimingSet,
}
