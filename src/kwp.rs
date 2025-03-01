use std::{io::Read, num::Wrapping};

use strum::FromRepr;

/// Maximum number of data bytes in a message (including the service ID)
pub const MAX_DATA_LENGTH: usize = u8::MAX as usize;
/// Maximum number of data bytes in message before the length byte is needed
pub const SHORT_DATA_LENGTH: usize = 0b00111111;

/// Decodes a message format byte into an address mode and a length
/// If length is None the message header will contain a length byte
fn decode_format(byte: u8) -> (AddressMode, Option<u8>) {
    let length = byte & 0b00111111;
    (
        match byte >> 6 {
            0b00 => AddressMode::None,
            0b01 => AddressMode::Carb,
            0b10 => AddressMode::Physical,
            0b11 => AddressMode::Functional,
            _ => panic!("impossible value"),
        },
        if length == 0 { None } else { Some(length) },
    )
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("specified length is bigger than received data")]
    NotEnoughData,
    #[error("given checksum does not match message contents")]
    InvalidChecksum,
    #[error("unknown service given")]
    InvalidService,
    #[error("io error")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct RawMessage {
    pub mode: AddressMode,
    pub target: Option<u8>,
    pub source: Option<u8>,
    pub service: Service,
    pub data: Vec<u8>,
}

impl RawMessage {
    pub fn new_simple_query(service: ServiceId, data: Vec<u8>) -> Self {
        Self::new_query(AddressMode::None, None, None, service, data)
    }
    pub fn new_query(
        mode: AddressMode,
        target: Option<u8>,
        source: Option<u8>,
        service: ServiceId,
        data: Vec<u8>,
    ) -> Self {
        // leave one byte for the service id
        assert!(data.len() < MAX_DATA_LENGTH);
        match mode {
            AddressMode::None => {
                assert!(target.is_none() && source.is_none());
            }
            _ => {
                assert!(target.is_some() && source.is_some());
            }
        }
        Self {
            mode,
            target,
            source,
            service: Service::Query(service),
            data,
        }
    }

    pub fn to_bytes(mut self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Include service id in length
        let length = 1 + self.data.len();

        let length_byte;

        if length <= SHORT_DATA_LENGTH {
            bytes.push(self.mode as u8 + length as u8);
            length_byte = None;
        } else {
            length_byte = Some(length as u8);
        }

        if self.mode != AddressMode::None {
            bytes.push(self.target.unwrap());
            bytes.push(self.source.unwrap());
        }

        if let Some(l) = length_byte {
            bytes.push(l);
        }

        bytes.push(self.service.into());

        bytes.append(&mut self.data);

        let crc: Wrapping<u8> = bytes.iter().map(|x| Wrapping(*x)).sum();

        bytes.push(crc.0);

        bytes
    }

    pub fn from_bytes<R: Read>(source: &mut R) -> Result<Self, Error> {
        // Buffer with enough space to hold an entire message, this includes:
        // - the one byte format header,
        // - the target address (optional),
        // - the source address (optional),
        // - the length byte (optional),
        // - the maximum of 255 data bytes,
        // - and the checksum byte.
        //
        // When the library is optimized for embedded devices, this will likely
        // be optimized so that allocating 260 bytes is not required every time
        // a message is received.
        let mut buf = [0; MAX_DATA_LENGTH + 5];

        source.read(&mut buf[0..1])?;

        let format = buf[0];

        let (mode, hlength) = decode_format(format);

        let target_addr;
        let source_addr;

        match mode {
            AddressMode::None => {
                target_addr = None;
                source_addr = None;
            }
            _ => {
                source.read(&mut buf[0..2])?;

                target_addr = Some(buf[0]);
                source_addr = Some(buf[1]);
            }
        }

        let length = if let Some(l) = hlength {
            l
        } else {
            source.read(&mut buf[0..1])?;
            buf[0]
        };

        source.read(&mut buf[0..1])?;

        let service = if let Some(id) = ServiceId::from_repr(buf[0]) {
            Ok(Service::Query(id))
        } else if let Some(r) = ServiceResponse::from_repr(buf[0]) {
            Ok(Service::Response(r))
        } else {
            Err(Error::InvalidService)
        }?;

        // remember length is 1 + data length (includes service id)
        let data = if length > 1 {
            let dbuf = &mut buf[0..(length as usize - 1)];
            source.read(dbuf).map_or(Err(Error::NotEnoughData), Ok)?;
            dbuf.iter().map(|b| *b).collect()
        } else {
            Vec::new()
        };

        source.read(&mut buf[0..1])?;

        let calc_crc: Wrapping<u8> = (&[format])
            .iter()
            .chain(target_addr.as_ref())
            .chain(source_addr.as_ref())
            .chain(if hlength.is_some() {
                None
            } else {
                Some(&length)
            })
            .chain(&[service.into()])
            .chain(&data)
            .map(|x| Wrapping(*x))
            .sum();

        if buf[0] != calc_crc.0 {
            return Err(Error::InvalidChecksum);
        }

        Ok(Self {
            mode,
            target: target_addr,
            source: source_addr,
            service,
            data,
        })
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressMode {
    None = 0b00000000,
    Carb = 0b01000000,
    Physical = 0b10000000,
    Functional = 0b11000000,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
pub enum DiagnosticMode {
    OBD = 0x81,
    EndOfLineVW = 0x83,
    EndOfLineBosch = 0x84,
    Programming = 0x85,
    Developer = 0x86,
    Diagnostics = 0x89,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
pub enum DataLinkError {
    /// Still working on previous request
    BusyRepeatRequest = 0x21,
    /// Processing not complete, still working on it
    RoutineNotComplete = 0x23,
    /// One or more parameter values is out of permitted range
    RequestOutOfRange = 0x31,
    /// Request received correctly, wait until final response is received befor sending another
    ResponsePending = 0x78,
    ScalingNotSupported = 0x91,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
pub enum ServiceError {
    /// Service not supported by control unit
    ServiceNotSupported = 0x11,
    /// Conditions for executing service not met, or interdependent services were sent in the wrong order
    ConditionsNotCorrect = 0x22,
    /// This service requires security access to be carried out successfully first
    SecurityAccessRequired = 0x33,
    InvalidKey = 0x35,
    /// Maximum security access failures has been reached
    TooManyAttempts = 0x36,
    /// Wait before making another security access request
    RequestingTooFast = 0x37,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
pub enum OtherError {
    GeneralReject = 0x10,
    FunctionNotSupported = 0x12,
    DownloadNotAccepted = 0x40,
    ImproperDownloadType = 0x41,
    CannotDownloadToAddress = 0x42,
    BadDownloadSize = 0x43,
    UploadNotAccepted = 0x50,
    ImproperUploadType = 0x51,
    CannotUploadFromAddress = 0x52,
    BadUploadSize = 0x53,
    TransferSuspended = 0x71,
    TransferAborted = 0x72,
    IllegalBlockTransferAddress = 0x74,
    IllegalBlockTransferSize = 0x75,
    IllegalBlockTransferType = 0x76,
    BlockTransferChecksumError = 0x77,
    IncorrectByteCountDuringBlockTransfer = 0x79,
    ServiceNotSupportedInActiveMode = 0x80,
    NoProgram = 0x90,
}

macro_rules! ServiceEnums {
    {$($(#[$attr:meta])? $name:ident = $id:expr => $response:expr),*} => {
        #[repr(u8)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
        pub enum ServiceId {
            $($(#[$attr])* $name = $id,)*
        }
        #[repr(u8)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
        pub enum ServiceResponse {
            NegativeResponse = 0x7F,
            $($(#[$attr])* $name = $response,)*
        }
    };
}

ServiceEnums! {
    RequestCurrentPowertrainDiagnosticData = 0x01 => 0x41,
    RequestPowertrainFreezeFrameData = 0x02 => 0x42,
    RequestEmissionRelatedDiagnosticInformation = 0x03 => 0x43,
    ClearResetEmissionRelatedDiagnosticInformation = 0x04 => 0x44,
    RequestOxygenSensorMonitoringTestResults = 0x05 => 0x45,
    RequestOnBoardMonitoringTestResultsForNoneContMonitoringSystem = 0x06 => 0x46,
    RequestOnBoardMonitoringTestResultsForContMonitoringSystem = 0x07 => 0x47,
    RequestControlOfOnBoardSystem = 0x08 => 0x48,
    RequestVehicleInformation = 0x09 => 0x49,

    StartDiagnosticSession = 0x10 => 0x50,
    ECUReset = 0x11 => 0x51,
    ReadFreezeFrameData = 0x12 => 0x52,
    ReadDiagnosticTroubleCodes = 0x13 => 0x53,
    ClearDiagnosticInformation = 0x14 => 0x54,

    ReadStatusOfDTC = 0x17 => 0x57,
    ReadDTCByStatus = 0x18 => 0x58,

    ReadECUIdentification = 0x1A => 0x5A,

    StopDiagnosticSession = 0x20 => 0x60,
    ReadDataByLocalIdentifier = 0x21 => 0x61,
    ReadDataByCommonIdentifier = 0x22 => 0x62,
    ReadMemoryByAddress = 0x23 => 0x63,

    SetDataRates = 0x26 => 0x66,
    SecurityAccess = 0x27 => 0x67,

    DynamicallyDefineLocalIdentifier = 0x2C => 0x6C,

    WriteDataByCommonIdentifier = 0x2E => 0x6E,
    InputOutputControlByCommonIdentifier = 0x2F => 0x6F,
    InputOutputControlByLocalIdentifier = 0x30 => 0x70,
    StartRoutineByLocalIdentifier = 0x31 => 0x71,
    StopRoutineByLocalIdentifier = 0x32 => 0x72,
    RequestRoutineResultsByLocalIdentifier = 0x33 => 0x73,
    RequestDownload = 0x34 => 0x74,
    RequestUpload = 0x35 => 0x75,
    TransferData = 0x36 => 0x76,
    RequestTransferExit = 0x37 => 0x77,
    StartRoutineByAddress = 0x38 => 0x78,
    StopRoutineByAddress = 0x39 => 0x79,
    ResquestRoutineResultsByAddress = 0x3A => 0x7A,
    WriteDataByLocalIdentifier = 0x3B => 0x7B,

    WriteMemoryByAddress = 0x3D => 0x7D,
    TesterPresent = 0x3E => 0x7E,

    /// Response is actually 0x7F (always negative)
    Reserved = 0x3F => 0xFF,

    ESCCode = 0x80 => 0xC0,
    StartCommunication = 0x81 => 0xC1,
    StopCommunication = 0x82 => 0xC2,
    AccessTimingParameter = 0x83 => 0xC3
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Query(ServiceId),
    Response(ServiceResponse),
}

impl TryFrom<u8> for Service {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if let Some(id) = ServiceId::from_repr(value) {
            Ok(Service::Query(id))
        } else if let Some(r) = ServiceResponse::from_repr(value) {
            Ok(Service::Response(r))
        } else {
            Err(Error::InvalidService)
        }
    }
}

impl Into<u8> for Service {
    fn into(self) -> u8 {
        match self {
            Service::Query(service_id) => service_id as u8,
            Service::Response(service_response) => service_response as u8,
        }
    }
}
