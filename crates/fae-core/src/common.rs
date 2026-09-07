use anyhow::{anyhow, Result};

pub fn to_word(x: u32) -> [u8; 4] {
    x.to_le_bytes()
}

pub fn get_word_from_slice(buf: &[u8], start_index_in_bytes: isize) -> Result<u32> {
    let len = buf.len() as isize;
    let start = if start_index_in_bytes < 0 {
        len + start_index_in_bytes
    } else {
        start_index_in_bytes
    };
    let end = start + 4;
    if start < 0 || end > len {
        return Err(anyhow!(
            "out-of-bounds word read at {} for size {}",
            start_index_in_bytes,
            buf.len()
        ));
    }
    let s = start as usize;
    Ok(u32::from_le_bytes([
        buf[s],
        buf[s + 1],
        buf[s + 2],
        buf[s + 3],
    ]))
}

pub fn round_up_pow2(x: usize, y: usize) -> usize {
    (x + y - 1) & !(y - 1)
}

pub fn get_bytes_text_suffix(value: usize) -> &'static str {
    if value > 1 {
        "bytes"
    } else {
        "byte"
    }
}
