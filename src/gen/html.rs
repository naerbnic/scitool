use super::doc::{text::RichText, ContentItem, Document, Section};

fn generate_rich_text(text: &RichText) -> maud::Markup {
    maud::html! {
        @for item in text.items() {
            @if item.style().bold() {
                b {
                    @if item.style().italic() {
                        i { (item.text()) }
                    } @else {
                        (item.text())
                    }
                }
            } @else if item.style().italic() {
                i { (item.text()) }
            } @else {
                (item.text())
            }

        }
    }
}

fn generate_plain_text(text: &RichText) -> maud::Markup {
    maud::html! {
        @for item in text.items() {
            (item.text())
        }
    }
}

fn generate_content(content: &super::doc::Content) -> maud::Markup {
    maud::html! {
        @for item in content.items() {
            @match item {
                ContentItem::Paragraph(text) => {
                    p { (generate_rich_text(text)) }
                }
                ContentItem::List(list) => {
                    ul {
                        @for item in list.items() {
                            li { (generate_content(item)) }
                        }
                    }
                }
                ContentItem::Dialogue(dialogue) => {
                    ul {
                        @for line in dialogue.lines() {
                            li {
                                span.speaker { (generate_rich_text(line.speaker())) }
                                ":"
                                span."line-text" { (generate_rich_text(line.line())) }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn generate_section(_level: usize, section: &Section) -> maud::Markup {
    maud::html! {
        h2 {
            (generate_rich_text(section.title()))
        }

        (generate_content(section.content()))

        @for subsection in section.subsections() {
            (generate_section(_level + 1, subsection))
        }
    }
}

pub fn generate_html(doc: &Document) -> anyhow::Result<String> {
    Ok(maud::html! {
        (maud::DOCTYPE)
        html {
            head {
                title { (generate_plain_text(doc.title())) }
            }
            body {
                h1 { (generate_rich_text(doc.title())) }
                @for chapter in doc.chapters() {
                    (generate_section(2, chapter))
                }
            }
        }
    }
    .into_string())
}
