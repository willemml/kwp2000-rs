use crate::Error;
use constants::ServiceId;
use message::Message;
use raw_message::RawMessage;
use response::Response;

pub mod client;
pub mod constants;
pub mod message;
pub mod raw_message;
pub mod response;

pub trait Interface {
    fn switch_baud(&mut self, baud_rate: u32) -> Result<(), Error>;
    fn send_raw(&mut self, message: RawMessage) -> Result<(), Error>;
    fn send(&mut self, message: Message) -> Result<(), Error> {
        self.send_raw(message.raw())
    }
    fn next_raw_message(&mut self) -> Result<RawMessage, Error>;

    /// Convenience function when not expecting to have to wait for a
    /// response
    fn next_response(&mut self) -> Result<Response, Error> {
        self.next_response_expect_wait(None)
    }

    /// Gets the next response type message, skips any query type messages
    /// (assumes any queries are echoes of the client from the server)
    /// Also waits if server replies with ResponsePending. This variant
    /// expects any response pending answers to match a specific service
    /// if `last_command` is not `None`.
    fn next_response_expect_wait(
        &mut self,
        last_command: Option<ServiceId>,
    ) -> Result<Response, Error> {
        loop {
            // TODO: Use timing parameters to sleep between reads
            let response = response::from_raw(self.next_raw_message()?)?;
            match response {
                Response::Echo(_) => continue,
                Response::StillProcessing(s) => {
                    if last_command.is_none() || last_command.is_some_and(|c| c == s) {
                        continue;
                    } else {
                        return Err(Error::UnexpectedPending);
                    }
                }
                _ => return Ok(response),
            }
        }
    }
}

/// https://github.com/NefMoto/NefMotoOpenSource/blob/9dfa4f32d9d68e0c9d32fed69a62a224c2f39d9f/Communication/KWP2000Actions.cs#L2583
pub fn security_key_from_seed(seed: [u8; 4]) -> u32 {
    let mut key = u32::from_be_bytes(seed);

    const EXT_RAM_KEY: u32 = 0x5FBD5DBD;

    const LOOP_COUNT: usize = 5;

    for _ in 0..LOOP_COUNT {
        if key >= 0x80000000 {
            key <<= 1;
            key |= 0x00000001;
            key ^= EXT_RAM_KEY;
        } else {
            key <<= 1;
        }
    }

    key
}

/// https://github.com/NefMoto/NefMotoOpenSource/blob/9dfa4f32d9d68e0c9d32fed69a62a224c2f39d9f/Communication/KWP2000Actions.cs#L560
pub fn baud_rate_to_byte(baud_rate: u32) -> u8 {
    let base = baud_rate * 32 / 6400;
    let mut best_scalar_distance = 1.0;
    let mut best_exp = 0;
    let mut best_exp_result = 1;
    for exp in (0..=7).rev() {
        let exp_result = 1 << exp;
        if exp_result < base {
            let scalar = base as f64 / exp_result as f64;
            if scalar < 64.0 && scalar > 32.0 {
                let scalar_distance = scalar - scalar.floor();
                if scalar_distance < best_scalar_distance {
                    best_scalar_distance = scalar_distance;
                    best_exp = exp;
                    best_exp_result = exp_result;
                }
            }
        }
    }

    let z = (base / best_exp_result) - 32;

    (((best_exp & 0x7) << 5) | (z & 0x1F)) as u8
}

/// https://github.com/NefMoto/NefMotoOpenSource/blob/9dfa4f32d9d68e0c9d32fed69a62a224c2f39d9f/Communication/KWP2000Actions.cs#L535
pub fn baud_rate_from_byte(byte: u8) -> u32 {
    let upper = (byte >> 5) & 7;
    let lower = (byte & 0x1F) as u32;

    let pow = 1 << upper;

    (pow * (lower + 32) * 6400) / 32
}
