#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextStyle {
    bold: bool,
    italic: bool,
}

impl TextStyle {
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
