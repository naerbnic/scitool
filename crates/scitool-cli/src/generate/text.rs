use scitool_book::{self as book, Control, FontControl, MessageSegment, MessageText};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextStyle {
    bold: bool,
    italic: bool,
}

impl TextStyle {
    pub fn of_italic() -> Self {
        let mut style = Self::default();
        style.set_italic(true);
        style
    }

    pub fn of_bold() -> Self {
        let mut style = Self::default();
        style.set_bold(true);
        style
    }

    pub fn bold(&self) -> bool {
        self.bold
    }

    pub fn italic(&self) -> bool {
        self.italic
    }

    pub fn set_bold(&mut self, bold: bool) -> &mut Self {
        self.bold = bold;
        self
    }

    pub fn set_italic(&mut self, italic: bool) -> &mut Self {
        self.italic = italic;
        self
    }
}

#[derive(Debug, Clone)]
pub struct TextItem {
    text: String,
    style: TextStyle,
}

impl TextItem {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn style(&self) -> &TextStyle {
        &self.style
    }
}

#[derive(Debug, Clone, Default)]
pub struct RichText {
    items: Vec<TextItem>,
}

impl RichText {
    pub fn from_msg_text(text: &MessageText) -> Self {
        let mut builder = RichText::builder();
        let mut curr_style = TextStyle::default();
        for segment in text.segments() {
            match segment {
                MessageSegment::Text(text) => {
                    builder.add_text(text, &curr_style);
                }
                MessageSegment::Control(ctrl) => match ctrl {
                    Control::Font(font_ctrl) => match font_ctrl {
                        FontControl::Default => curr_style = TextStyle::default(),
                        // Italic Control Sequences
                        FontControl::Italics => curr_style = TextStyle::of_italic(),
                        // Bold Control Sequences
                        FontControl::SuperLarge | FontControl::Title | FontControl::BoldLike => {
                            curr_style = TextStyle::of_bold()
                        }
                        // Ignored
                        FontControl::Lowercase | FontControl::Unknown => {}
                    },
                    Control::Color(_) => {
                        // We ignore color control sequences for now.
                    }
                },
            }
        }
        builder.build()
    }
    pub fn items(&self) -> &[TextItem] {
        &self.items
    }

    pub fn builder() -> RichTextBuilder {
        RichTextBuilder {
            output: RichText::default(),
        }
    }
}

impl<T> From<T> for RichText
where
    T: ToString,
{
    fn from(text: T) -> Self {
        RichText {
            items: vec![TextItem {
                text: text.to_string(),
                style: TextStyle::default(),
            }],
        }
    }
}

pub struct RichTextBuilder {
    output: RichText,
}

impl RichTextBuilder {
    pub fn add_plain_text(&mut self, text: impl ToString) -> &mut Self {
        self.add_text(text, &TextStyle::default())
    }

    pub fn add_text(&mut self, text: impl ToString, curr_style: &TextStyle) -> &mut Self {
        match self.output.items.last_mut() {
            Some(last) if &last.style == curr_style => {
                last.text.push_str(text.to_string().as_str());
            }
            _ => {
                self.output.items.push(TextItem {
                    text: text.to_string(),
                    style: curr_style.clone(),
                });
            }
        }
        self
    }

    pub fn build(self) -> RichText {
        self.output
    }
}

pub fn make_room_title(room: &book::Room<'_>) -> RichText {
    let mut room_title_builder = RichText::builder();
    room_title_builder.add_plain_text(room.name()).add_text(
        format!(" (Room #{:?})", room.id().room_num()),
        &TextStyle::of_italic(),
    );
    room_title_builder.build()
}

pub fn make_conversation_title(conv: &book::Conversation<'_>) -> RichText {
    RichText::from(match (conv.verb(), conv.condition()) {
        (Some(verb), Some(cond)) => format!(
            "On {} ({})",
            verb.name(),
            cond.desc()
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("Condition #{:?}", cond.id().condition_num()))
        ),
        (Some(verb), None) => format!("On {}", verb.name()),
        (None, Some(cond)) => format!(
            "When {}",
            cond.desc()
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("Condition #{:?}", cond.id().condition_num()))
        ),
        (None, None) => "On Any".to_string(),
    })
}

pub fn make_noun_title(noun: &book::Noun<'_>) -> RichText {
    let mut noun_desc = noun
        .desc()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("Noun #{:?}", noun.id().noun_num()));

    if noun.is_cutscene() {
        noun_desc.push_str(" (Cutscene)");
    }
    RichText::from(noun_desc)
}
