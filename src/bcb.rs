//! Mostly a re-implementation of what was done in the NefMoto Flasher software.
//! The biggest difference is that my implementation should be zero copy.
//!
//! This is Bosch BCB Type 1 compression with a simple XOR encryption.
//! ME7 uses "GEHEIM" (secret in German) as the encryption key.
//! See here on NefMoto: http://nefariousmotorsports.com/forum/index.php?topic=23501.msg169475#msg169475
//!
//! The header byte on the first data transfer of a sector is put before the compressed data and then it is all encrypted together.
//!
//! This post says that BCB Type 1 is similar to Nintendo's LZSS. But it's quite different
//! because LZSS uses a dictionary of sequences while BCB only compresses immediately repeating
//! bytes. There may be some useful information in the thread so I will link it anyways.
//! http://nefariousmotorsports.com/forum/index.php?topic=6583.msg123050#msg123050
//!
//! See link for original code:
//! https://github.com/NefMoto/NefMotoOpenSource/blob/9dfa4f32d9d68e0c9d32fed69a62a224c2f39d9f/Communication/KWP2000Actions.cs#L3383

use std::io::Write;

use crate::Error;

pub const BLOCK_HEADER_SIZE: usize = 2;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum RepeatMode {
    NoRepeats = 0,
    Repeating = 1,
    RepeatingAlso = 2,
    Unknown = 3,
}

pub fn encrypt_and_compress(
    mut max_len: usize,
    data: &[u8],
    key_index: &mut usize,
    key: &[u8],
    is_first: bool,
) -> Result<(usize, Vec<u8>), Error> {
    if is_first {
        max_len -= 2;
    }

    let (uncompressed_length, mut compressed) = create_bcb_data(data, max_len)?;

    if is_first {
        let mut new = vec![0x1A, 0x01];
        new.append(&mut compressed);
        compressed = new;
    }

    encrypt_data(key, &mut compressed, key_index)?;

    Ok((uncompressed_length, compressed))
}

pub fn encrypt_data(key: &[u8], data: &mut [u8], key_index: &mut usize) -> Result<(), Error> {
    for b in data.iter_mut() {
        *b = *b ^ key[*key_index];
        *key_index += 1;
        if *key_index >= key.len() {
            *key_index = 0;
        }
    }

    Ok(())
}

pub fn create_bcb_data(data: &[u8], max_len: usize) -> Result<(usize, Vec<u8>), Error> {
    let mut current_index = 0;

    let mut compressed = Vec::new();

    let mut uncompressed_total = 0;

    while (max_len - compressed.len()) > 4 && current_index < data.len() {
        let uncompressed = next_bcb_block(
            max_len - compressed.len(),
            &mut current_index,
            data,
            &mut compressed,
        )?;

        current_index += uncompressed;
        uncompressed_total += uncompressed;
    }

    Ok((uncompressed_total, compressed))
}

pub fn next_bcb_block<W: Write>(
    max_len: usize,
    current_index: &mut usize,
    data: &[u8],
    compressed: &mut W,
) -> Result<usize, Error> {
    const MAX_REPEATS: usize = 0x1000;
    const MIN_REPEATS: usize = 4;

    let max_data_bytes = Ord::min(max_len - BLOCK_HEADER_SIZE, data.len());
    let max_index_norepeats = Ord::min(max_len - BLOCK_HEADER_SIZE, data.len());

    let mut repeat_start = 0;
    let mut repeat_end = 0;

    let mut found_repeat = false;

    if max_index_norepeats > *current_index + 1 {
        for x in *current_index..max_index_norepeats {
            if data[x] == data[x + 1] {
                repeat_start = x;

                let max_repeat_index = Ord::min(repeat_start + MAX_REPEATS, data.len());

                for y in (repeat_start + 1)..max_repeat_index {
                    if data[repeat_start] == data[y] {
                        if y % 2 == 1 {
                            if found_repeat || y - repeat_start + 1 >= MIN_REPEATS {
                                repeat_end = y;
                                found_repeat = true;
                            }
                        }
                    } else {
                        break;
                    }
                }
                if found_repeat {
                    break;
                }
            }
        }
    }

    Ok(if found_repeat && repeat_start == *current_index {
        let repeated_bytes = repeat_end - repeat_start + 1;

        if repeated_bytes > 0 {
            let repeat_mode = RepeatMode::Repeating as u16;
            let header = repeat_mode << 14 | (0x3FFF & repeated_bytes as u16);

            compressed.write(&header.to_be_bytes())?;
            compressed.write(&[data[repeat_start]])?;

            repeated_bytes
        } else {
            0
        }
    } else {
        let data_bytes = if found_repeat {
            repeat_start - *current_index
        } else {
            max_data_bytes - (max_data_bytes % 2)
        };

        if data_bytes > 0 {
            let repeat_mode = RepeatMode::NoRepeats as u16;
            let header = repeat_mode << 14 | (0x3FFF & data_bytes as u16);

            compressed.write(&header.to_be_bytes())?;
            compressed.write(&data[*current_index..data_bytes])?;

            data_bytes
        } else {
            0
        }
    })
}
