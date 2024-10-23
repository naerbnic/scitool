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

pub fn signed_extend_byte(byte: u8) -> u16 {
    byte as i8 as i16 as u16
}

pub fn unsigned_extend_byte(byte: u8) -> u16 {
    byte as u16
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