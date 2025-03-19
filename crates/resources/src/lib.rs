pub mod file;
pub mod types;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, clap::ValueEnum)]
#[repr(u8)]
pub enum ResourceType {
    View = 0x80,
    Pic,
    #[clap(alias = "scr")]
    Script,
    #[clap(alias = "txt")]
    Text,
    Sound,
    Memory,
    #[clap(alias = "voc")]
    Vocab,
    Font,
    Cursor,
    Patch,
    Bitmap,
    Palette,
    CdAudio,
    Audio,
    Sync,
    #[clap(alias = "msg")]
    Message,
    Map,
    Heap,
    Audio36,
    Sync36,
    Translation,
    Rave,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("Conversion Error: {0}")]
pub struct ConversionError(String);

impl ResourceType {
    pub fn from_file_ext(ext: &str) -> Result<Self, ConversionError> {
        match ext.to_lowercase().as_str() {
            "v56" => Ok(ResourceType::View),
            "p56" => Ok(ResourceType::Pic),
            "scr" => Ok(ResourceType::Script),
            "tex" => Ok(ResourceType::Text),
            "snd" => Ok(ResourceType::Sound),
            "voc" => Ok(ResourceType::Vocab),
            "fon" => Ok(ResourceType::Font),
            "cur" => Ok(ResourceType::Cursor),
            "pat" => Ok(ResourceType::Patch),
            "bit" => Ok(ResourceType::Bitmap),
            "pal" => Ok(ResourceType::Palette),
            "cda" => Ok(ResourceType::CdAudio),
            "aud" => Ok(ResourceType::Audio),
            "syn" => Ok(ResourceType::Sync),
            "msg" => Ok(ResourceType::Message),
            "map" => Ok(ResourceType::Map),
            "hep" => Ok(ResourceType::Heap),
            "trn" => Ok(ResourceType::Translation),
            _ => Err(ConversionError(format!(
                "Invalid file extension for resource type: {}",
                ext
            ))),
        }
    }

    // This may need to be given a target engine type to be correct.
    pub fn to_file_ext(&self) -> &'static str {
        match self {
            ResourceType::View => "v56",
            ResourceType::Pic => "p56",
            ResourceType::Script => "scr",
            ResourceType::Text => "tex",
            ResourceType::Sound => "snd",
            ResourceType::Vocab => "voc",
            ResourceType::Font => "fon",
            ResourceType::Cursor => "cur",
            ResourceType::Patch => "pat",
            ResourceType::Bitmap => "bit",
            ResourceType::Palette => "pal",
            ResourceType::CdAudio => "cda",
            ResourceType::Audio => "aud",
            ResourceType::Sync => "syn",
            ResourceType::Message => "msg",
            ResourceType::Map => "map",
            ResourceType::Heap => "hep",
            ResourceType::Translation => "trn",
            _ => "",
        }
    }
}

impl TryFrom<u8> for ResourceType {
    type Error = ConversionError;

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
            _ => Err(ConversionError(format!(
                "Invalid resource type: 0x{:02X}",
                value
            ))),
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
    type_id: ResourceType,
    resource_num: u16,
}

impl ResourceId {
    pub fn new(type_id: ResourceType, resource_num: u16) -> ResourceId {
        ResourceId {
            type_id,
            resource_num,
        }
    }

    pub fn type_id(&self) -> ResourceType {
        self.type_id
    }

    pub fn resource_num(&self) -> u16 {
        self.resource_num
    }
}

impl std::fmt::Debug for ResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}:{:}", self.type_id, self.resource_num)
    }
}
