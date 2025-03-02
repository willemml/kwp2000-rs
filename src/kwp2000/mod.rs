pub mod constants;
pub mod message;
pub mod security;

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

/// https://github.com/NefMoto/NefMotoOpenSource/blob/9dfa4f32d9d68e0c9d32fed69a62a224c2f39d9f/Communication/KWP2000Actions.cs#L2583
pub fn security_key_from_seed(seed: &[u8]) -> Vec<u8> {
    assert_eq!(seed.len(), 4);

    let mut key: u64 = seed
        .iter()
        .map(|n| *n as u64)
        .enumerate()
        .rev()
        .map(|(i, b)| b << i * 8)
        .sum();

    const EXT_RAM_KEY: u64 = 0x5FBD5DBD;

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

    key.to_le_bytes().into()
}
