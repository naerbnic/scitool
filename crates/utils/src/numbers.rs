pub mod bit_convert;

/// Returns the number as a signed byte if it can be safely narrowed.
pub fn safe_signed_narrow(number: u16) -> anyhow::Result<u8> {
    let sign_part = number & 0xFF80;
    if sign_part != 0 && sign_part != 0xFF80 {
        anyhow::bail!(
            "number {} cannot be safely narrowed to a signed byte",
            number
        );
    }
    Ok((number & 0xFF) as u8)
}

pub fn safe_unsigned_narrow(number: u16) -> anyhow::Result<u8> {
    if number & 0xFF00 != 0 {
        anyhow::bail!(
            "number {} cannot be safely narrowed to an unsigned byte",
            number
        );
    }
    Ok((number & 0xFF) as u8)
}

#[must_use]
#[expect(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
pub fn signed_extend_byte(byte: u8) -> u16 {
    i16::from(byte as i8) as u16
}

#[must_use]
pub fn unsigned_extend_byte(byte: u8) -> u16 {
    u16::from(byte)
}

pub fn read_byte<R: std::io::Read>(mut buf: R) -> anyhow::Result<u8> {
    let mut byte = [0];
    buf.read_exact(&mut byte)?;
    Ok(byte[0])
}

pub fn read_word<R: std::io::Read>(mut buf: R) -> anyhow::Result<u16> {
    let mut bytes = [0; 2];
    buf.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

pub fn write_byte<W: std::io::Write>(mut buf: W, byte: u8) -> anyhow::Result<()> {
    buf.write_all(&[byte])?;
    Ok(())
}

pub fn write_word<W: std::io::Write>(mut buf: W, word: u16) -> anyhow::Result<()> {
    buf.write_all(&word.to_le_bytes())?;
    Ok(())
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn safe_narrow_from_isize(size: isize) -> anyhow::Result<u16> {
    let top_bits_mask = !0x7FFFisize;
    let top_bits = size & top_bits_mask;
    anyhow::ensure!(
        top_bits == 0 || top_bits == top_bits_mask,
        "number {:?} cannot be safely narrowed to a signed word",
        size
    );
    Ok(size as usize as u16)
}

#[must_use]
pub fn read_u16_le_from_slice(slice: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(slice[at..][..2].try_into().unwrap())
}

pub fn write_u16_le_to_slice(slice: &mut [u8], at: usize, value: u16) {
    slice[at..][..2].copy_from_slice(&value.to_le_bytes());
}

pub fn modify_u16_le_in_slice(
    slice: &mut [u8],
    at: usize,
    body: impl FnOnce(u16) -> anyhow::Result<u16>,
) -> anyhow::Result<()> {
    let slice: &mut [u8] = &mut slice[at..][..2];
    let slice: &mut [u8; 2] = slice.try_into()?;
    let value = u16::from_le_bytes(*slice);
    let new_value = body(value)?;
    slice.copy_from_slice(&new_value.to_le_bytes());
    Ok(())
}
