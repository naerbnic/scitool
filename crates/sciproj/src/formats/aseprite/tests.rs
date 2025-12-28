#![cfg(test)]
use crate::formats::aseprite::{ColorDepth, SpriteBuilder};
use scidev::utils::block::{Block, BlockBuilderFactory, MemBlock};

#[test]
fn test_sprite_to_block_serialization() {
    // 1. Create a Sprite using the Builder
    let mut builder = SpriteBuilder::new(ColorDepth::Rgba);
    builder.set_width(32);
    builder.set_height(32);

    // Add a frame
    let mut frame_builder = builder.add_frame();
    frame_builder.set_duration(100);
    let frame_idx = frame_builder.index();

    // Add a layer
    let mut layer_builder = builder.add_layer();
    layer_builder.set_name("Layer 1");
    // We know we added at index 0
    let layer_idx = layer_builder.index();

    // Add a cel with some pixel data
    let mut pixels = vec![0u8; 32 * 32 * 4];
    // Fill with red
    for i in 0..(32 * 32) {
        pixels[i * 4] = 255; // R
        pixels[i * 4 + 1] = 0; // G
        pixels[i * 4 + 2] = 0; // B
        pixels[i * 4 + 3] = 255; // A
    }

    {
        let mut cel_builder = builder.add_cel(layer_idx, frame_idx);
        let block = Block::from(MemBlock::from_vec(pixels));
        cel_builder.set_image(32, 32, block);
    }

    let sprite = builder.build().expect("Failed to build sprite");

    // 2. Serialize using to_block
    let factory = BlockBuilderFactory::new_in_memory();
    let result = sprite.to_block(&factory);

    // 3. functional verification
    assert!(result.is_ok(), "Serialization failed: {:?}", result.err());

    let block = result.unwrap();
    // Minimally check size. Header (128) + Frame Header (16) + ...
    assert!(block.len() > 144, "Block too small: {}", block.len());
}
