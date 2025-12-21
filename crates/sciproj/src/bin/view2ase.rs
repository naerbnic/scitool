use anyhow::Result;
use bytes::{Buf as _, BufMut};
use clap::Parser;
use scidev::{
    resources::{
        ResourceId, ResourceSet, ResourceType,
        types::{
            palette::{self, Palette},
            view::{CelEntry, LoopEntry, ViewHeader},
        },
    },
    utils::{
        block::{Block, MemBlock},
        buffer::SplittableBuffer,
        mem_reader::{BufferMemReader, MemReader},
    },
};
use sciproj::formats::aseprite::{
    ChunkBlock, Header, HeaderFlags, build_frame_block,
    cel::{CelChunk, CelType, RawCel},
    layer::{BlendMode, LayerChunk, LayerFlags, LayerType},
    palette::{PaletteChunk, PaletteEntry, PaletteEntryFlags},
    tags::{AnimationDirection, Tag, TagsChunk},
};
use std::fs::File;
use std::path::PathBuf;

#[derive(Parser)]
struct Args {
    /// Path to the game root directory
    game_dir: PathBuf,

    /// Number of the view resource to convert
    view_number: u16,

    /// Output Aseprite file path
    output_file: PathBuf,
}

#[derive(Debug)]
struct ViewMetadata {
    header: ViewHeader,
    loops: Vec<LoopMetadata>,
}

impl ViewMetadata {
    fn from_data<B>(data: &B) -> Result<Self>
    where
        B: SplittableBuffer,
    {
        let mut reader = BufferMemReader::new(data.as_fallible());
        let header = ViewHeader::read_from(&mut reader)?;
        let loop_count = usize::from(header.loop_count);
        let loop_size = usize::from(header.loop_size);
        let mut loop_reader = reader.read_to_subreader("loop_data", loop_count * loop_size)?;

        let mut loops = Vec::with_capacity(loop_count);
        for i in 0..loop_count {
            let loop_entry = LoopEntry::read_from(
                &mut loop_reader.read_to_subreader(format!("{i}"), loop_size)?,
            )?;

            // The cel data is indexed from the start of the loop data
            let cel_count = usize::from(loop_entry.cel_count);
            let cel_size = usize::from(header.cel_size);
            let cel_offset = usize::try_from(loop_entry.cel_offset).unwrap();
            let cel_data = data
                .sub_buffer(cel_offset..)
                .sub_buffer(..cel_count * cel_size);
            let mut cel_reader = BufferMemReader::new(cel_data.into_fallible());

            let mut cels = Vec::with_capacity(cel_count);
            for i in 0..cel_count {
                cels.push(CelEntry::read_from(
                    &mut cel_reader.read_to_subreader(format!("{i}"), cel_size)?,
                )?);
            }
            loops.push(LoopMetadata {
                entry: loop_entry,
                cels,
            });
        }
        Ok(Self { header, loops })
    }
}

#[derive(Debug)]
struct LoopMetadata {
    entry: LoopEntry,
    cels: Vec<CelEntry>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Load ResourceSet and find the View
    println!("Loading resources from {}", args.game_dir.display());
    let resources = ResourceSet::from_root_dir(&args.game_dir)?;
    let view_id = ResourceId::new(ResourceType::View, args.view_number);
    let view_res = resources
        .get_resource(&view_id)
        .ok_or_else(|| anyhow::anyhow!("View resource {} not found", args.view_number))?;

    println!("Found view {}. Parsing...", args.view_number);
    let data = view_res.data();
    println!("View data size: {}", data.len());

    let full_data = data.open_mem(..)?;

    let view_metadata = ViewMetadata::from_data(&full_data)?;

    println!("View Metadata: {view_metadata:#?}");

    let palette = if view_metadata.header.pal_offset != 0 {
        let palette_data =
            full_data.sub_buffer(usize::try_from(view_metadata.header.pal_offset).unwrap()..);
        Some(Palette::from_data(palette_data)?)
    } else {
        None
    };

    // 4. Construct Aseprite file
    let mut max_width = 0;
    let mut max_height = 0;
    let mut total_frames = 0;

    for cel in view_metadata.loops.iter().flat_map(|l| &l.cels) {
        if cel.width > max_width {
            max_width = cel.width;
        }
        if cel.height > max_height {
            max_height = cel.height;
        }
        total_frames += 1;
    }

    println!("Max size: {max_width}x{max_height}, Total frames: {total_frames}");

    let mut ase_header = Header {
        file_size: 0, // Will be filled later
        frames_count: u16::try_from(total_frames).unwrap(),
        width: max_width,
        height: max_height,
        color_depth: 8, // Indexed
        flags: HeaderFlags::HAS_LAYER_OPACITY,
        transparent_index: 255,
        num_indexed_colors: 256,
        pixel_width: 1,
        pixel_height: 1,
        grid_x: 0,
        grid_y: 0,
        grid_width: 16,
        grid_height: 16,
        reserved2: [0; 84],
    };

    let mut frames_data = Vec::new();
    let mut frame_cursor = 0;
    let mut tags = Vec::new();

    for (loop_idx, loop_metadata) in view_metadata.loops.iter().enumerate() {
        let start_frame = frame_cursor;

        for cel in &loop_metadata.cels {
            // Decode the pixels
            let pixels = decode_rle(cel, &full_data);

            // Create Frame
            let mut chunks = Vec::new();

            if frame_cursor == 0 {
                // Define Layer
                chunks.push(ChunkBlock::from_value(LayerChunk {
                    flags: LayerFlags::VISIBLE | LayerFlags::EDITABLE,
                    layer_type: LayerType::Normal,
                    child_level: 0,
                    blend_mode: BlendMode::Normal,
                    opacity: 255,
                    layer_name: "Wheee 1".to_string(),
                    uuid: None,
                    default_width: 0,
                    default_height: 0,
                }));

                // Add Palette
                if let Some(palette) = &palette
                    && !palette.is_empty()
                {
                    let mut pal_entries = Vec::new();

                    let num_entries = palette.len();
                    let default_entry = palette::PaletteEntry::new(0, 0, 0);
                    for i in palette.range() {
                        let entry = palette.get(i).unwrap_or(&default_entry);
                        pal_entries.push(PaletteEntry {
                            flags: PaletteEntryFlags::empty(),
                            red: entry.red(),
                            green: entry.green(),
                            blue: entry.blue(),
                            alpha: 255,
                            name: None,
                        });
                    }
                    chunks.push(ChunkBlock::from_value(PaletteChunk {
                        new_palette_size: u32::try_from(num_entries).unwrap(),
                        first_color_index: u32::from(palette.first_color()),
                        last_color_index: u32::from(palette.last_color()),
                        entries: pal_entries,
                    }));
                }
            }

            // Cel Chunk
            chunks.push(ChunkBlock::from_value(CelChunk {
                layer_index: 0,
                x: 0, // Should be based on displacement
                y: 0,
                opacity: 255,
                cel_type: CelType::Raw(RawCel {
                    width: cel.width,
                    height: cel.height,
                    pixels: pixels.clone(),
                }),
                z_index: 0,
                reserved: [0; 5],
            }));

            frames_data.push(chunks);
            frame_cursor += 1;
        }

        tags.push(Tag {
            from_frame: u16::try_from(start_frame).unwrap(),
            to_frame: u16::try_from(frame_cursor - 1).unwrap(),
            direction: AnimationDirection::Forward,
            repeat: 0,
            name: format!("Loop {loop_idx}"),
        });
    }

    // Insert Tags chunk into Frame 0
    frames_data[0].push(ChunkBlock::from_value(TagsChunk { tags }));

    let frames_blocks = Block::concat(
        frames_data
            .into_iter()
            .map(|chunks| build_frame_block(100, chunks))
            .collect::<Vec<_>>(),
    );

    // Rewrite Main Header
    ase_header.file_size = u32::try_from(frames_blocks.len() + 128).unwrap();

    let header_block = ase_header.to_block();

    let file_block = Block::concat([header_block, frames_blocks]);

    // Write to file
    {
        let mut file = File::create(&args.output_file)?;
        std::io::copy(&mut file_block.open_reader(..)?, &mut file)?;
    }

    println!("Written to {}", args.output_file.display());

    Ok(())
}

fn decode_rle(cel_entry: &CelEntry, res_data: &MemBlock) -> Vec<u8> {
    // println!("Readling cel: {cel_entry:?}");
    let num_pixels = usize::from(cel_entry.width) * usize::from(cel_entry.height);
    let mut pixels = bytes::BytesMut::with_capacity(num_pixels).limit(num_pixels); // Initialize with transparent
    // SCI1.1 RLE decoder
    //
    // We potentially have to track two different pieces of data: The RLE stream and the literal stream.
    // If there is no literal stream, the literal data is encoded in the RLE stream.
    let rle_data = res_data.sub_buffer(usize::try_from(cel_entry.rle_offset).unwrap()..);
    let literal_data = if cel_entry.literal_offset > 0 {
        Some(res_data.sub_buffer(usize::try_from(cel_entry.literal_offset).unwrap()..))
    } else {
        None
    };

    // Given that we're only reading raw bytes, it's easier to use byte slices for parsing here.
    let mut rle_data = &rle_data[..];
    let mut literal_data = literal_data.as_ref().map(|d| &d[..]);

    while pixels.has_remaining_mut() {
        let code = rle_data.get_u8();
        let has_high_bit = code & 0x80 != 0;
        if !has_high_bit {
            // Copy
            // The first 7 bits are the run length for copy operations. Since
            // the first bit is zero, we can just use the code as the run length.
            let run_length = usize::from(code);
            // println!("\tCopy: {run_length}, {}", pixels.remaining_mut());
            let mut src = if let Some(literal_data) = literal_data.as_mut() {
                literal_data
            } else {
                &mut rle_data
            };

            let copy_bytes = bytes::Buf::take(&mut src, run_length);
            pixels.put(copy_bytes);
            continue;
        }

        // This is some flavor of RLE.
        let action = code & 0x40;
        let run_length = usize::from(code & 0x3F);

        let color = if action == 0 {
            // Fill operation. Take fill color from available data.
            if let Some(literal_data) = literal_data.as_mut() {
                literal_data.get_u8()
            } else {
                rle_data.get_u8()
            }
        } else {
            // Skip (Transparent). Use the clear key.
            cel_entry.clear_key
        };
        pixels.put_bytes(color, run_length);
    }

    pixels.into_inner().into()
}
