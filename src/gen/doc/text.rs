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

    #[expect(dead_code)]
    pub fn builder() -> RichTextBuilder {
        RichTextBuilder {
            output: Some(RichText::default()),
        }
    }
}

pub struct RichTextBuilder {
    output: Option<RichText>,
}

impl RichTextBuilder {
    #[expect(dead_code)]
    pub fn add_plain_text(&mut self, text: impl ToString) -> &mut Self {
        self.add_text(text, TextStyle::default())
    }

    pub fn add_text(&mut self, text: impl ToString, curr_style: TextStyle) -> &mut Self {
        match self.output.as_mut().unwrap().items.last_mut() {
            Some(last) if last.style == curr_style => {
                last.text.push_str(text.to_string().as_str());
            }
            _ => {
                self.output.as_mut().unwrap().items.push(TextItem {
                    text: text.to_string(),
                    style: curr_style,
                });
            }
        }
        self
    }

    #[expect(dead_code)]
    pub fn build(mut self) -> RichText {
        self.output.take().unwrap()
    }
}
