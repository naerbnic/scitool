pub mod audio36;
pub mod file;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, clap::ValueEnum)]
#[repr(u8)]
pub enum ResourceType {
    View = 0x80,
    Pic,
    Script,
    Text,
    Sound,
    Memory,
    Vocab,
    Font,
    Cursor,
    Patch,
    Bitmap,
    Palette,
    CdAudio,
    Audio,
    Sync,
    Message,
    Map,
    Heap,
    Audio36,
    Sync36,
    Translation,
    Rave,
}

impl TryFrom<u8> for ResourceType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x80 => Ok(ResourceType::View),
            0x81 => Ok(ResourceType::Pic),
            0x82 => Ok(ResourceType::Script),
            0x83 => Ok(ResourceType::Text),
            0x84 => Ok(ResourceType::Sound),
            0x85 => Ok(ResourceType::Memory),
            0x86 => Ok(ResourceType::Vocab),
            0x87 => Ok(ResourceType::Font),
            0x88 => Ok(ResourceType::Cursor),
            0x89 => Ok(ResourceType::Patch),
            0x8A => Ok(ResourceType::Bitmap),
            0x8B => Ok(ResourceType::Palette),
            0x8C => Ok(ResourceType::CdAudio),
            0x8D => Ok(ResourceType::Audio),
            0x8E => Ok(ResourceType::Sync),
            0x8F => Ok(ResourceType::Message),
            0x90 => Ok(ResourceType::Map),
            0x91 => Ok(ResourceType::Heap),
            0x92 => Ok(ResourceType::Audio36),
            0x93 => Ok(ResourceType::Sync36),
            0x94 => Ok(ResourceType::Translation),
            0x95 => Ok(ResourceType::Rave),
            _ => Err(format!("Invalid resource type: 0x{:02X}", value)),
        }
    }
}

impl From<ResourceType> for u8 {
    fn from(value: ResourceType) -> u8 {
        value as u8
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId {
    pub type_id: ResourceType,
    pub resource_num: u16,
}

impl ResourceId {
    pub fn new(type_id: ResourceType, resource_num: u16) -> ResourceId {
        ResourceId {
            type_id,
            resource_num,
        }
    }
}

impl std::fmt::Debug for ResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}:{:}", self.type_id, self.resource_num)
    }
}
