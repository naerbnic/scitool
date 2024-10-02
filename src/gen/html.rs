use super::doc::{text::RichText, ContentItem, Document, Section};

const SCRIPT_CSS: &str = include_str!("script.css");

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
                    .dialogue {
                        @for line in dialogue.lines() {
                            .line id=(line.id()){
                                .speaker { (generate_rich_text(line.speaker())) ":" }
                                ."line-text" { (generate_rich_text(line.line())) }
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
        .section id=[section.id()] {
            ."section-title" {
                (generate_rich_text(section.title()))
            }

            (generate_content(section.content()))

            @for subsection in section.subsections() {
                (generate_section(_level + 1, subsection))
            }
        }
    }
}

pub fn generate_html(doc: &Document) -> anyhow::Result<String> {
    Ok(maud::html! {
        (maud::DOCTYPE)
        html {
            head {
                title { (generate_plain_text(doc.title())) }
                style { (SCRIPT_CSS) }
            }
            body {
                h1 { (generate_rich_text(doc.title())) }
                @for chapter in doc.chapters() {
                    (generate_section(0, chapter))
                }
            }
        }
    }
    .into_string())
}
