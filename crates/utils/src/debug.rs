/// Print a hex dump of the given data. The `initial_offset` is
/// what the first byte of the data should be considered as, for printing
/// of offsets.
pub fn hex_dump(data: &[u8], initial_offset: usize) {
    // We want to print out an output like this:
    //      00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F
    //      -----------------------------------------------
    // 0000 01 23 45 67 89 AB CD EF 01 23 45 67 89 AB CD EF
    // 0010 01 23 45 67 89 AB CD EF 01 23 45 67 89 AB CD EF

    // Find the length of the offset in characters
    // We subtraact one, because the final byte will be at offset
    // length - 1.
    let max_offset = initial_offset + data.len() - 1;

    let num_offset_hex_chars = ((max_offset.next_power_of_two() - 1).trailing_ones() / 4) as usize;

    let offset_padding = " ".repeat(num_offset_hex_chars);

    let mut remaining_data = data;
    let mut curr_offset = 0;

    let mut num_lines = 0;

    while !remaining_data.is_empty() {
        if num_lines % 16 == 0 {
            println!(
                "{}   -----------------------------------------------",
                offset_padding
            );
            println!(
                "{}   00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F",
                offset_padding
            );
            println!(
                "{}   -----------------------------------------------",
                offset_padding
            );
        }
        // The position of the first byte shown in the current line
        let line_start = (curr_offset + initial_offset) % 16;
        let line_end = std::cmp::min(remaining_data.len(), 16 - line_start) + line_start;
        let line_length = line_end - line_start;

        let empty_hex_prefix = "   ".repeat(line_start);
        let empty_hex_suffix = "   ".repeat(16 - line_end);
        let line_hex = remaining_data[..line_length]
            .iter()
            .map(|b| format!("{:02X} ", b))
            .collect::<Vec<_>>()
            .join("");
        let empty_ascii_prefix = " ".repeat(line_start);
        let empty_ascii_suffix = " ".repeat(16 - line_end);
        let line_ascii = remaining_data[..line_length]
            .iter()
            .map(|b| {
                if *b >= 32 && *b <= 126 {
                    *b as char
                } else {
                    '.'
                }
            })
            .collect::<String>();

        let offset_text = format!("{:0num_offset_hex_chars$X}", curr_offset + initial_offset);

        println!(
            "{}: {}{}{} {}{}{}",
            offset_text,
            empty_hex_prefix,
            line_hex,
            empty_hex_suffix,
            empty_ascii_prefix,
            line_ascii,
            empty_ascii_suffix
        );
        remaining_data = &remaining_data[line_length..];
        curr_offset += line_length;
        num_lines += 1;
    }
}
