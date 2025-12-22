use anyhow::Result;
use clap::Parser;
use scidev::{
    resources::{
        ResourceId, ResourceSet, ResourceType,
        types::{
            palette::{self},
            view::{Loop, View},
        },
    },
    utils::block::Block,
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

    let view = View::from_resource(data)?;
    // let view = View::from_resource(&Block::from_mem_block(data.open_mem(..).unwrap()))?;

    let palette = view.palette();

    // 4. Construct Aseprite file
    let mut max_width = 0;
    let mut max_height = 0;
    let mut total_frames = 0;

    for cel in view.loops().iter().flat_map(Loop::cels) {
        if cel.width() > max_width {
            max_width = cel.width();
        }
        if cel.height() > max_height {
            max_height = cel.height();
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

    for (loop_idx, loop_metadata) in view.loops().iter().enumerate() {
        let start_frame = frame_cursor;

        for cel in loop_metadata.cels() {
            // Decode the pixels
            let pixels = cel.decode_pixels()?;

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
                    width: cel.width(),
                    height: cel.height(),
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
