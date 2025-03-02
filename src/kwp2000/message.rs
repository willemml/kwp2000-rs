use std::io::Read;
use std::num::Wrapping;

use super::Error;
use super::constants::*;

/// Maximum number of data bytes in a message (including the service ID)
pub const MAX_DATA_LENGTH: usize = u8::MAX as usize;
/// Maximum number of data bytes in message before the length byte is needed
pub const SHORT_DATA_LENGTH: usize = 0b00111111;

/// Decodes a message format byte into an address mode and a length
/// If length is None the message header will contain a length byte
pub fn decode_format(byte: u8) -> (AddressMode, Option<u8>) {
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

#[derive(Debug, Clone)]
pub struct RawMessage {
    pub mode: AddressMode,
    pub target: Option<u8>,
    pub source: Option<u8>,
    pub service: Service,
    pub data: Vec<u8>,
}

impl RawMessage {
    /// Creates a message using the one byte header mode
    pub fn new_query_none(service: ServiceId, data: Vec<u8>) -> Self {
        Self::new_query(AddressMode::None, None, None, service, data)
    }
    /// Creates a message using physical addressing
    pub fn new_query_physical(service: ServiceId, target: u8, source: u8, data: Vec<u8>) -> Self {
        Self::new_query(
            AddressMode::Physical,
            Some(target),
            Some(source),
            service,
            data,
        )
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

        source.read_exact(&mut buf[0..1])?;

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
                source.read_exact(&mut buf[0..2])?;

                target_addr = Some(buf[0]);
                source_addr = Some(buf[1]);
            }
        }

        let length = if let Some(l) = hlength {
            l
        } else {
            source.read_exact(&mut buf[0..1])?;
            buf[0]
        };

        source.read_exact(&mut buf[0..1])?;

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
            source
                .read_exact(dbuf)
                .map_or(Err(Error::NotEnoughData), Ok)?;
            dbuf.iter().map(|b| *b).collect()
        } else {
            Vec::new()
        };

        source.read_exact(&mut buf[0..1])?;

        let crc_calc: Wrapping<u8> = (&[format])
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
        if buf[0] != crc_calc.0 {
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
