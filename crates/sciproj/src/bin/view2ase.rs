// IGNORE THIS FILE UNTIL THE ASEPRITE REWRITE IS COMPLETED

use anyhow::Result;
use clap::Parser;
use scidev::{
    resources::{
        ResourceId, ResourceSet, ResourceType,
        types::view::{Loop, View},
    },
    utils::block::{Block, BlockBuilderFactory},
};
use sciproj::formats::aseprite::{
    AnimationDirection, Color, ColorDepth, LayerFlags, PaletteEntry, SpriteBuilder,
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

    let mut builder = SpriteBuilder::new(ColorDepth::Indexed(256));
    builder.set_transparent_color(255);
    builder.set_width(max_width);
    builder.set_height(max_height);

    // Create a single layer for the view
    let mut layer_builder = builder.add_layer();
    layer_builder.set_name("Layer 1");
    layer_builder.set_flags(LayerFlags::VISIBLE | LayerFlags::EDITABLE);
    let layer_index = layer_builder.index();

    let mut frame_cursor = 0;

    for (loop_idx, loop_metadata) in view.loops().iter().enumerate() {
        let start_frame = frame_cursor;

        for cel in loop_metadata.cels() {
            // Decode the pixels
            let pixels = cel.decode_pixels()?;

            // Add Frame
            let mut frame_builder = builder.add_frame();
            frame_builder.set_duration(100); // Default duration?
            let frame_index = frame_builder.index();

            // Add Cel to Layer 1
            let mut cel_builder = builder.add_cel(layer_index, frame_index);
            // Default position to (0,0) or center? View cels usually have displacement.
            // For now, let's just put them at 0,0, but we might want to use displacement x/y later if available in Cel
            cel_builder.set_position(0, 0);

            // We need to convert pixels to Block or similar for set_image
            // set_image takes (width, height, Into<Block>)

            // Pixel Remapping for Round-Trip Transparency
            // Swap 'clear_key' with 255 (Global Transparent)
            let mut remapped_pixels = pixels.clone();
            let clear_key = cel.clear_key();
            let global_transparent = 255u8;

            for pixel in &mut remapped_pixels {
                if *pixel == clear_key {
                    *pixel = global_transparent;
                } else if *pixel == global_transparent {
                    *pixel = clear_key;
                }
            }

            cel_builder.set_image(cel.width(), cel.height(), Block::from_vec(remapped_pixels));

            // Store original transparency key in UserData
            cel_builder.set_extension_property(
                "scidev/scitool",
                "transparency_key",
                sciproj::formats::aseprite::Property::U8(clear_key),
            );

            frame_cursor += 1;
        }

        // Add Tag for the loop
        builder.add_tag(
            u32::try_from(start_frame).unwrap(),
            u32::try_from(frame_cursor - 1).unwrap(),
            format!("Loop {loop_idx}"),
            AnimationDirection::Forward,
        );
    }

    // Set Palette
    if let Some(palette) = &palette
        && !palette.is_empty()
    {
        let mut pal_entries = Vec::new();
        let default_entry = scidev::resources::types::palette::PaletteEntry::new(0, 0, 0);

        for i in palette.range() {
            let entry = palette.get(i).unwrap_or(&default_entry);
            pal_entries.push(PaletteEntry::new(
                Color::from_rgba(entry.red(), entry.green(), entry.blue(), 255),
                None,
            ));
        }

        builder.set_palette(pal_entries);
    }

    let sprite = builder.build()?;

    // Write to file
    let factory = BlockBuilderFactory::new_in_memory();
    let file_block = sprite.to_block(&factory)?;

    {
        let mut file = File::create(&args.output_file)?;
        std::io::copy(&mut file_block.open_reader(..).unwrap(), &mut file)?;
    }

    println!("Written to {}", args.output_file.display());

    Ok(())
}
