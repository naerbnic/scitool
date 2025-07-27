use scitool_book::{self as book, Control, FontControl, MessageSegment, MessageText};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TextStyle {
    bold: bool,
    italic: bool,
}

impl TextStyle {
    pub(crate) fn of_italic() -> Self {
        let mut style = Self::default();
        style.set_italic(true);
        style
    }

    pub(crate) fn of_bold() -> Self {
        let mut style = Self::default();
        style.set_bold(true);
        style
    }

    pub(crate) fn bold(&self) -> bool {
        self.bold
    }

    pub(crate) fn italic(&self) -> bool {
        self.italic
    }

    pub(crate) fn set_bold(&mut self, bold: bool) -> &mut Self {
        self.bold = bold;
        self
    }

    pub(crate) fn set_italic(&mut self, italic: bool) -> &mut Self {
        self.italic = italic;
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TextItem {
    text: String,
    style: TextStyle,
}

impl TextItem {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn style(&self) -> &TextStyle {
        &self.style
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RichText {
    items: Vec<TextItem>,
}

impl RichText {
    pub(crate) fn from_msg_text(text: &MessageText) -> Self {
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
                            curr_style = TextStyle::of_bold();
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

    pub(crate) fn items(&self) -> &[TextItem] {
        &self.items
    }

    pub(crate) fn builder() -> RichTextBuilder {
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

pub(crate) struct RichTextBuilder {
    output: RichText,
}

impl RichTextBuilder {
    pub(crate) fn add_plain_text(&mut self, text: &impl ToString) -> &mut Self {
        self.add_text(text, &TextStyle::default())
    }

    pub(crate) fn add_rich_text(&mut self, text: &RichText) -> &mut Self {
        for item in text.items() {
            self.add_text(&item.text(), item.style());
        }
        self
    }

    pub(crate) fn add_text(&mut self, text: &impl ToString, curr_style: &TextStyle) -> &mut Self {
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

    pub(crate) fn build(self) -> RichText {
        self.output
    }
}

pub(crate) fn make_room_title(room: &book::Room<'_>) -> RichText {
    let mut room_title_builder = RichText::builder();
    room_title_builder.add_plain_text(&room.name());
    room_title_builder.build()
}

pub(crate) fn make_conversation_title(conv: &book::Conversation<'_>) -> RichText {
    RichText::from(match (conv.verb(), conv.condition()) {
        (Some(verb), Some(condition)) => format!(
            "On {} ({})",
            verb.name(),
            condition.desc().map_or_else(
                || format!("Condition #{:?}", condition.id().condition_num()),
                ToString::to_string
            )
        ),
        (Some(verb), None) => format!("On {}", verb.name()),
        (None, Some(condition)) => format!(
            "When {}",
            condition.desc().map_or_else(
                || format!("Condition #{:?}", condition.id().condition_num()),
                ToString::to_string
            )
        ),
        (None, None) => "On Any".to_string(),
    })
}

pub(crate) fn make_noun_title(noun: &book::Noun<'_>) -> RichText {
    let mut noun_desc = noun.desc().map_or_else(
        || format!("Noun #{:?}", noun.id().noun_num()),
        ToOwned::to_owned,
    );

    if noun.is_cutscene() {
        noun_desc.push_str(" (Cutscene)");
    }
    RichText::from(noun_desc)
}
