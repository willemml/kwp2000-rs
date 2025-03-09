use crate::Error;
use strum::FromRepr;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicDefinitionMode {
    DefineByLocalIdentifier = 0x01,
    DefineByCommonIdentifier = 0x02,
    DefineByMemoryAddress = 0x03,
    ClearDynamicallyDefinedLocalIdentifier = 0x04,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRepr)]
pub enum TimingParameter {
    Limits = 0,
    Defaults = 1,
    Read = 2,
    Set = 3,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionFormat {
    Uncompressed = 0x00,
    Bosch = 0x10,
    Hitachi = 0x20,
    Marelli = 0x30,
    Lucas = 0x40,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionFormat {
    Unencrypted = 0x00,
    Bosch = 0x01,
    Hitachi = 0x02,
    Marelli = 0x03,
    Lucas = 0x04,
}

pub const fn data_format_byte(compression: CompressionFormat, encryption: EncryptionFormat) -> u8 {
    compression as u8 | encryption as u8
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMode {
    Single = 0x01,
    Slow = 0x02,
    Medium = 0x03,
    Fast = 0x04,
    Stop = 0x05,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromRepr)]
pub enum SecurityLevel {
    Seed1 = 0x01,
    Seed2 = 0x03,
    Seed3 = 0x05,
    Seed4 = 0x07,
    Key1 = 0x02,
    Key2 = 0x04,
    Key3 = 0x06,
    Key4 = 0x08,
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

    /// Data link errors

    /// Still working on previous request
    BusyRepeatRequest = 0x21,
    /// Processing not complete, still working on it
    RoutineNotComplete = 0x23,
    /// One or more parameter values is out of permitted range
    RequestOutOfRange = 0x31,
    /// Request received correctly, wait until final response is received befor sending another
    ResponsePending = 0x78,
    ScalingNotSupported = 0x91,

    /// Other errors
    GeneralReject = 0x10,
    FunctionNotSupportedOrInvalidFormat = 0x12,
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

// service name = hexword query => hexword response
//
// NegativeResponse = 0x7F is always added to the response list
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
