use super::doc::{ContentItem, Document, Section, text::RichText};

const GOOGLE_ICONS_LINK: maud::PreEscaped<&str> = maud::PreEscaped(
    r#"<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200" />"#,
);
const SCRIPT_CSS: maud::PreEscaped<&str> = maud::PreEscaped(include_str!("script.css"));
const SCRIPT_JS: maud::PreEscaped<&str> = maud::PreEscaped(include_str!("script.js"));

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

fn generate_link_button(elem_id: &str) -> maud::Markup {
    maud::html! {
        ."link-button".button "data-linkid"=(elem_id) {
            div."material-symbols-outlined" { "link" }
        }
    }
}

fn generate_copy_button(copy_text: &str) -> maud::Markup {
    maud::html! {
        ."copy-button".button "data-copytext"=(copy_text) {
            div."material-symbols-outlined" { "content_copy" }
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
                                ."line-text" { (generate_rich_text(line.line()))
                                  ."hover-reveal" {
                                    (generate_link_button(line.id()))
                                    (generate_copy_button(line.id()))
                                  }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn generate_section(_level: usize, section: &Section) -> maud::Markup {
    maud::html! {
        .section id=[section.id()] {
            ."section-title" {
                (generate_rich_text(section.title()))
                @if let Some(id) = section.id() {
                    ."hover-reveal" {
                        (generate_link_button(id))
                        (generate_copy_button(id))
                    }
                }
            }
            ."section-body" {

                (generate_content(section.content()))

                @for subsection in section.subsections() {
                    (generate_section(_level + 1, subsection))
                }
            }
        }
    }
}

pub fn generate_html(doc: &Document) -> anyhow::Result<String> {
    Ok(maud::html! {
        (maud::DOCTYPE)
        html {
            head {
                (GOOGLE_ICONS_LINK)
                style { (SCRIPT_CSS) }
                title { (generate_plain_text(doc.title())) }
            }
            body lang="en-US"{
                h1 { (generate_rich_text(doc.title())) }
                @for chapter in doc.chapters() {
                    (generate_section(0, chapter))
                }
                script { (SCRIPT_JS) }
            }
        }
    }
    .into_string())
}
