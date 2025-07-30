use crate::{Control, FontControl, MessageSegment, MessageText};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextStyle {
    bold: bool,
    italic: bool,
}

impl TextStyle {
    #[must_use]
    pub fn of_plain() -> Self {
        TextStyle {
            bold: false,
            italic: false,
        }
    }

    #[must_use]
    pub fn of_italic() -> Self {
        let mut style = Self::default();
        style.set_italic(true);
        style
    }

    #[must_use]
    pub(crate) fn of_bold() -> Self {
        let mut style = Self::default();
        style.set_bold(true);
        style
    }

    #[must_use]
    pub fn bold(&self) -> bool {
        self.bold
    }

    #[must_use]
    pub fn italic(&self) -> bool {
        self.italic
    }

    #[must_use]
    pub fn is_plain(&self) -> bool {
        !self.bold && !self.italic
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
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn style(&self) -> &TextStyle {
        &self.style
    }
}

#[derive(Debug, Clone, Default)]
pub struct RichText {
    items: Vec<TextItem>,
}

impl RichText {
    #[must_use]
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

    #[must_use]
    pub fn items(&self) -> &[TextItem] {
        &self.items
    }

    #[must_use]
    pub fn builder() -> RichTextBuilder {
        RichTextBuilder {
            output: RichText::default(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.iter().all(|item| item.text.is_empty())
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
    pub fn add_plain_text(&mut self, text: &impl ToString) -> &mut Self {
        self.add_text(text, &TextStyle::default())
    }

    pub fn add_rich_text(&mut self, text: &RichText) -> &mut Self {
        for item in text.items() {
            self.add_text(&item.text(), item.style());
        }
        self
    }

    pub fn add_text(&mut self, text: &impl ToString, curr_style: &TextStyle) -> &mut Self {
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

    #[must_use]
    pub fn build(self) -> RichText {
        self.output
    }
}
