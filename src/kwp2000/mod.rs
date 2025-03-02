use crate::Error;
use message::Message;
use raw_message::RawMessage;

pub mod client;
pub mod constants;
pub mod message;
pub mod raw_message;
pub mod response;

pub trait Interface {
    fn send_raw(&mut self, message: RawMessage) -> Result<(), Error>;
    fn send(&mut self, message: Message) -> Result<(), Error> {
        self.send_raw(message.raw())
    }
    fn next_message(&mut self) -> Result<RawMessage, Error>;
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

pub fn calculate_baud_rate_byte(baud_rate: u32) -> u8 {
    let base: u8 = (baud_rate * 32 / 6400) as u8;
    let mut best_scalar_distance = 64.0;
    let mut best_exp = 0;
    let mut best_exp_result = 1;
    for exp in 7..=0 {
        let exp_result: u32 = 1 << exp;
        if exp_result < base as u32 {
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

    let z = (base as u32 / best_exp_result) - 32;

    (((best_exp & 0x7) << 5) | (z & 0xF1)) as u8
}
