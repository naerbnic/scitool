//! Traits and implementations used to generate documents, including VO scripts.

use text::RichText;

pub mod text;

fn push_last_mut<T>(vec: &mut Vec<T>, value: T) -> &mut T {
    vec.push(value);
    vec.last_mut().unwrap()
}

pub struct Document {
    title: RichText,
    chapters: Vec<Section>,
}

impl Document {
    pub fn title(&self) -> &RichText {
        &self.title
    }

    pub fn chapters(&self) -> &[Section] {
        &self.chapters
    }
}

pub struct Section {
    title: RichText,
    content: Content,
    subsections: Vec<Section>,
}

impl Section {
    pub fn title(&self) -> &RichText {
        &self.title
    }

    pub fn content(&self) -> &Content {
        &self.content
    }

    pub fn subsections(&self) -> &[Section] {
        &self.subsections
    }
}

impl Section {
    fn with_title(title: RichText) -> Self {
        Self {
            title,
            content: Content::new(),
            subsections: Vec::new(),
        }
    }
}

pub struct List {
    items: Vec<Content>,
}

impl List {
    pub fn items(&self) -> &[Content] {
        &self.items
    }
}

impl List {
    fn new() -> Self {
        Self { items: Vec::new() }
    }
}

pub struct Line {
    speaker: RichText,
    line: RichText,
}

impl Line {
    pub fn speaker(&self) -> &RichText {
        &self.speaker
    }

    pub fn line(&self) -> &RichText {
        &self.line
    }
}

pub struct Dialogue {
    lines: Vec<Line>,
}

impl Dialogue {
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }
}

impl Dialogue {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }
}

pub enum ContentItem {
    Paragraph(RichText),
    List(List),
    Dialogue(Dialogue),
}

pub struct Content {
    items: Vec<ContentItem>,
}

impl Content {
    pub fn items(&self) -> &[ContentItem] {
        &self.items
    }
}

/// Private operations
impl Content {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    fn push_paragraph(&mut self, text: RichText) {
        self.items.push(ContentItem::Paragraph(text));
    }

    fn push_list_mut(&mut self) -> &mut Vec<Content> {
        let last = push_last_mut(&mut self.items, ContentItem::List(List::new()));
        let ContentItem::List(list) = last else {
            panic!("Expected last item to be a list");
        };
        &mut list.items
    }

    fn push_dialogue_mut(&mut self) -> &mut Vec<Line> {
        let last = push_last_mut(&mut self.items, ContentItem::Dialogue(Dialogue::new()));
        let ContentItem::Dialogue(dialogue) = last else {
            panic!("Expected last item to be a dialogue");
        };
        &mut dialogue.lines
    }
}

pub struct DocumentBuilder {
    document: Document,
}

impl DocumentBuilder {
    pub fn new(title: impl Into<RichText>) -> Self {
        Self {
            document: Document {
                title: title.into(),
                chapters: Vec::new(),
            },
        }
    }

    pub fn add_chapter(&mut self, title: impl Into<RichText>) -> SectionBuilder {
        SectionBuilder {
            section: push_last_mut(
                &mut self.document.chapters,
                Section::with_title(title.into()),
            ),
        }
    }

    pub fn build(self) -> Document {
        self.document
    }
}

pub struct ListBuilder<'a> {
    list: &'a mut Vec<Content>,
}

impl ListBuilder<'_> {
    #[expect(dead_code)]
    fn add_item(&mut self) -> ContentBuilder {
        ContentBuilder {
            content: push_last_mut(self.list, Content::new()),
        }
    }
}

pub struct SectionBuilder<'a> {
    section: &'a mut Section,
}

impl<'a> SectionBuilder<'a> {
    pub fn add_content(&mut self) -> ContentBuilder {
        ContentBuilder {
            content: &mut self.section.content,
        }
    }

    pub fn into_section_builder(self) -> SubSectionBuilder<'a> {
        SubSectionBuilder {
            section: self.section,
        }
    }
}

pub struct SubSectionBuilder<'a> {
    section: &'a mut Section,
}

impl SubSectionBuilder<'_> {
    pub fn add_subsection(&mut self, title: impl Into<RichText>) -> SectionBuilder {
        SectionBuilder {
            section: push_last_mut(
                &mut self.section.subsections,
                Section::with_title(title.into()),
            ),
        }
    }
}

pub struct ContentBuilder<'a> {
    content: &'a mut Content,
}

impl ContentBuilder<'_> {
    #[expect(dead_code)]
    pub fn add_paragraph(&mut self, text: RichText) {
        self.content.push_paragraph(text);
    }

    #[expect(dead_code)]
    pub fn add_list(&mut self) -> ListBuilder {
        ListBuilder {
            list: self.content.push_list_mut(),
        }
    }

    pub fn add_dialogue(&mut self) -> DialogueBuilder {
        DialogueBuilder {
            dialogue: self.content.push_dialogue_mut(),
        }
    }
}

pub struct DialogueBuilder<'a> {
    dialogue: &'a mut Vec<Line>,
}

impl DialogueBuilder<'_> {
    pub fn add_line(&mut self, speaker: impl Into<RichText>, line: impl Into<RichText>) {
        self.dialogue.push(Line {
            speaker: speaker.into(),
            line: line.into(),
        });
    }
}
