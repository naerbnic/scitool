//! Traits and implementations used to generate documents, including VO scripts.

use super::text::RichText;

fn push_last_mut<T>(vec: &mut Vec<T>, value: T) -> &mut T {
    vec.push(value);
    vec.last_mut().unwrap()
}

pub(crate) struct Document {
    title: RichText,
    chapters: Vec<Section>,
}

impl Document {
    pub(crate) fn title(&self) -> &RichText {
        &self.title
    }

    pub(crate) fn chapters(&self) -> &[Section] {
        &self.chapters
    }
}

pub(crate) struct Section {
    title: RichText,
    id: Option<String>,
    content: Content,
    subsections: Vec<Section>,
}

impl Section {
    pub(crate) fn title(&self) -> &RichText {
        &self.title
    }

    pub(crate) fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub(crate) fn content(&self) -> &Content {
        &self.content
    }

    pub(crate) fn subsections(&self) -> &[Section] {
        &self.subsections
    }
}

impl Section {
    fn with_title(title: RichText) -> Self {
        Self {
            title,
            id: None,
            content: Content::new(),
            subsections: Vec::new(),
        }
    }
}

pub(crate) struct List {
    items: Vec<Content>,
}

impl List {
    pub(crate) fn items(&self) -> &[Content] {
        &self.items
    }
}

impl List {
    fn new() -> Self {
        Self { items: Vec::new() }
    }
}

pub(crate) struct Line {
    speaker: RichText,
    id: String,
    text: RichText,
}

impl Line {
    pub(crate) fn speaker(&self) -> &RichText {
        &self.speaker
    }

    pub(crate) fn text(&self) -> &RichText {
        &self.text
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }
}

pub(crate) struct Dialogue {
    lines: Vec<Line>,
}

impl Dialogue {
    pub(crate) fn lines(&self) -> &[Line] {
        &self.lines
    }
}

impl Dialogue {
    fn new() -> Self {
        Self { lines: Vec::new() }
    }
}

pub(crate) enum ContentItem {
    Paragraph(RichText),
    List(List),
    Dialogue(Dialogue),
}

pub(crate) struct Content {
    items: Vec<ContentItem>,
}

impl Content {
    pub(crate) fn items(&self) -> &[ContentItem] {
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
        let ContentItem::List(last_list) = last else {
            panic!("Expected last item to be a list");
        };
        &mut last_list.items
    }

    fn push_dialogue_mut(&mut self) -> &mut Vec<Line> {
        let last = push_last_mut(&mut self.items, ContentItem::Dialogue(Dialogue::new()));
        let ContentItem::Dialogue(dialogue) = last else {
            panic!("Expected last item to be a dialogue");
        };
        &mut dialogue.lines
    }
}

pub(crate) struct DocumentBuilder {
    document: Document,
}

impl DocumentBuilder {
    pub(crate) fn new(title: impl Into<RichText>) -> Self {
        Self {
            document: Document {
                title: title.into(),
                chapters: Vec::new(),
            },
        }
    }

    pub(crate) fn add_chapter(&mut self, title: impl Into<RichText>) -> SectionBuilder {
        SectionBuilder {
            section: push_last_mut(
                &mut self.document.chapters,
                Section::with_title(title.into()),
            ),
        }
    }

    pub(crate) fn build(self) -> Document {
        self.document
    }
}

pub(crate) struct ListBuilder<'a> {
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

pub(crate) struct SectionBuilder<'a> {
    section: &'a mut Section,
}

impl<'a> SectionBuilder<'a> {
    pub(crate) fn set_id(&mut self, id: impl Into<String>) {
        self.section.id = Some(id.into());
    }

    pub(crate) fn add_content(&mut self) -> ContentBuilder {
        ContentBuilder {
            content: &mut self.section.content,
        }
    }

    pub(crate) fn into_section_builder(self) -> SubSectionBuilder<'a> {
        SubSectionBuilder {
            section: self.section,
        }
    }
}

pub(crate) struct SubSectionBuilder<'a> {
    section: &'a mut Section,
}

impl SubSectionBuilder<'_> {
    pub(crate) fn add_subsection(&mut self, title: impl Into<RichText>) -> SectionBuilder {
        SectionBuilder {
            section: push_last_mut(
                &mut self.section.subsections,
                Section::with_title(title.into()),
            ),
        }
    }
}

pub(crate) struct ContentBuilder<'a> {
    content: &'a mut Content,
}

impl ContentBuilder<'_> {
    pub(crate) fn add_paragraph(&mut self, text: impl Into<RichText>) {
        self.content.push_paragraph(text.into());
    }

    #[expect(dead_code)]
    pub(crate) fn add_list(&mut self) -> ListBuilder {
        ListBuilder {
            list: self.content.push_list_mut(),
        }
    }

    pub(crate) fn add_dialogue(&mut self) -> DialogueBuilder {
        DialogueBuilder {
            dialogue: self.content.push_dialogue_mut(),
        }
    }
}

pub(crate) struct DialogueBuilder<'a> {
    dialogue: &'a mut Vec<Line>,
}

impl DialogueBuilder<'_> {
    pub(crate) fn add_line(
        &mut self,
        speaker: impl Into<RichText>,
        line: impl Into<RichText>,
        id: String,
    ) {
        self.dialogue.push(Line {
            speaker: speaker.into(),
            id,
            text: line.into(),
        });
    }
}
