fn write_dump_header<W: std::io::Write>(mut out: W, padding_spaces: usize) -> std::io::Result<()> {
    let offset_padding = " ".repeat(padding_spaces);
    writeln!(
        out,
        "{}  -----------------------------------------------",
        offset_padding
    )?;
    writeln!(
        out,
        "{}  00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F",
        offset_padding
    )?;
    writeln!(
        out,
        "{}  -----------------------------------------------",
        offset_padding
    )?;
    Ok(())
}

fn write_dump_line<W: std::io::Write>(
    mut out: W,
    offset_width: usize,
    offset: usize,
    data: &[u8],
) -> std::io::Result<(usize, &[u8])> {
    let line_start = offset % 16;
    let line_end = std::cmp::min(data.len(), 16 - line_start) + line_start;
    let line_length = line_end - line_start;

    let empty_hex_prefix = "   ".repeat(line_start);
    let empty_hex_suffix = "   ".repeat(16 - line_end);
    let line_hex = data[..line_length]
        .iter()
        .map(|b| format!("{:02X} ", b))
        .collect::<Vec<_>>()
        .join("");
    let empty_ascii_prefix = " ".repeat(line_start);
    let empty_ascii_suffix = " ".repeat(16 - line_end);
    let line_ascii = data[..line_length]
        .iter()
        .map(|b| {
            if *b >= 32 && *b <= 126 {
                *b as char
            } else {
                '.'
            }
        })
        .collect::<String>();

    let offset_text = format!("{:0offset_width$X}", offset, offset_width = offset_width);

    writeln!(
        out,
        "{}: {}{}{} {}{}{}",
        offset_text,
        empty_hex_prefix,
        line_hex,
        empty_hex_suffix,
        empty_ascii_prefix,
        line_ascii,
        empty_ascii_suffix
    )?;
    Ok((offset + line_length, &data[line_length..]))
}

/// Print a hex dump of the given data to stdout. The `initial_offset` is
/// what the first byte of the data should be considered as, for printing
/// of offsets.
pub fn hex_dump(data: &[u8], initial_offset: usize) {
    hex_dump_to(std::io::stdout(), data, initial_offset).unwrap();
}

/// Print a hex dump of the given data to the output writer. The `initial_offset` is
/// what the first byte of the data should be considered as, for printing
/// of offsets.
pub fn hex_dump_to<W: std::io::Write>(
    mut out: W,
    data: &[u8],
    initial_offset: usize,
) -> std::io::Result<()> {
    // We want to print out an output like this:
    //       -----------------------------------------------
    //       00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
    //       -----------------------------------------------
    // 0000: 01 23 45 67 89 AB CD EF 01 23 45 67 89 AB CD EF
    // 0010: 01 23 45 67 89 AB CD EF 01 23 45 67 89 AB CD EF

    // Find the length of the offset in characters
    // We subtraact one, because the final byte will be at offset
    // length - 1.
    let max_offset = initial_offset + data.len();

    let num_visible_bits = (max_offset.next_power_of_two() - 1).trailing_ones();

    let num_offset_hex_chars = ((num_visible_bits + 3) / 4) as usize;

    let mut remaining_data = data;
    let mut curr_offset = 0;

    let mut num_lines = 0;

    while !remaining_data.is_empty() {
        if num_lines % 16 == 0 {
            write_dump_header(&mut out, num_offset_hex_chars)?;
        }
        (curr_offset, remaining_data) =
            write_dump_line(&mut out, num_offset_hex_chars, curr_offset, remaining_data)?;
        num_lines += 1;
    }
    Ok(())
}
