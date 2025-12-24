use std::collections::{BTreeMap, btree_map};

use crate::formats::aseprite::{
    BlendMode, CelIndex, ColorDepth, LayerFlags, LayerType, UserData,
    backing::{CelContents, FrameContents, LayerContents, SpriteContents},
};

pub struct FrameBuilder<'a> {
    index: u32,
    contents: &'a mut FrameContents,
}

pub struct LayerBuilder<'a> {
    contents: &'a mut LayerContents,
}

pub struct CelBuilder<'a> {
    index: CelIndex,
    contents: &'a mut CelContents,
}

pub struct SpriteBuilder {
    contents: SpriteContents,
}

impl SpriteBuilder {
    pub fn new(color_mode: ColorDepth) -> Self {
        Self {
            contents: SpriteContents {
                color_depth: color_mode,
                width: 0,
                height: 0,
                pixel_height: 1,
                pixel_width: 1,
                transparent_color: 0,
                frames: Vec::new(),
                layers: Vec::new(),
                tags: Vec::new(),
                cels: BTreeMap::new(),
                external_files: Vec::new(),
            },
        }
    }

    pub fn add_frame(&mut self) -> FrameBuilder {
        let frame_index = self.contents.frames.len();
        let frame = FrameContents {
            duration_ms: 0,
            palette: None,
        };
        self.contents.frames.push(frame);
        FrameBuilder {
            index: u32::try_from(frame_index).unwrap(),
            contents: &mut self.contents.frames[frame_index],
        }
    }

    pub fn add_layer(&mut self) -> LayerBuilder {
        let layer_index = self.contents.layers.len();
        let layer = LayerContents {
            name: String::new(),
            flags: LayerFlags::empty(),
            layer_type: LayerType::Normal,
            blend_mode: BlendMode::Normal,
            opacity: 255,
            uuid: None,
            user_data: UserData::default(),
        };
        self.contents.layers.push(layer);
        LayerBuilder {
            contents: &mut self.contents.layers[layer_index],
        }
    }

    pub fn add_cel(&mut self, layer: u32, frame: u32) -> CelBuilder {
        let index = CelIndex { layer, frame };
        let cel_ref = match self.contents.cels.entry(index) {
            btree_map::Entry::Vacant(vac) => vac.insert(CelContents {
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                pixel_data: Vec::new(),
                user_data: UserData::default(),
            }),
            btree_map::Entry::Occupied(occ) => occ.into_mut(),
        };
        CelBuilder {
            index,
            contents: cel_ref,
        }
    }

    pub fn set_transparent_color(&mut self, color: u8) {
        self.contents.transparent_color = color;
    }

    pub fn set_width(&mut self, width: u16) {
        self.contents.width = width;
    }

    pub fn set_height(&mut self, height: u16) {
        self.contents.height = height;
    }

    pub fn set_pixel_ratio(&mut self, pixel_width: u8, pixel_height: u8) {
        self.contents.pixel_width = pixel_width;
        self.contents.pixel_height = pixel_height;
    }
}
