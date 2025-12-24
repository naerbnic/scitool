use crate::formats::aseprite::{ColorDepth, backing::SpriteContents};

pub struct Sprite {
    pub(super) contents: SpriteContents,
}

impl Sprite {
    #[must_use]
    pub fn width(&self) -> u16 {
        self.contents.width
    }

    #[must_use]
    pub fn height(&self) -> u16 {
        self.contents.height
    }

    #[must_use]
    pub fn pixel_width(&self) -> u8 {
        self.contents.pixel_width
    }

    #[must_use]
    pub fn pixel_height(&self) -> u8 {
        self.contents.pixel_height
    }

    #[must_use]
    pub fn color_depth(&self) -> ColorDepth {
        self.contents.color_depth
    }
}
