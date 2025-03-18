#![expect(dead_code)]

use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

fn ends_with_newline(s: &str) -> bool {
    s.ends_with('\n') | s.ends_with('\r')
}
fn ends_with_blankline(s: &str) -> bool {
    for c in s.chars().rev() {
        match c {
            ' ' | '\t' => continue,
            '\n' | '\r' => return true,
            _ => return false,
        }
    }
    true
}

struct NewlineChars<'a>(std::iter::Peekable<std::str::Chars<'a>>);

impl<'a> NewlineChars<'a> {
    fn new(s: &'a str) -> Self {
        NewlineChars(s.chars().peekable())
    }
}

impl Iterator for NewlineChars<'_> {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            Some('\r') => {
                let _ = self.0.next_if(|c| c == &'\n');
                Some('\n')
            }
            Some(c) => Some(c),
            None => None,
        }
    }
}

struct StringBuilder<'a> {
    state: IndentState<'a>,
    output: &'a mut String,
}

impl<'a> StringBuilder<'a> {
    pub fn with_mut_string(output: &'a mut String) -> Self {
        StringBuilder {
            state: IndentState::Top,
            output,
        }
    }
    pub fn ensure_newline(&mut self) {
        if !ends_with_blankline(self.output) {
            self.add_newline();
        }
    }

    pub fn add_newline(&mut self) {
        self.output.push('\n');
        self.state.append_prefix(self.output);
    }

    pub fn add_raw_text(&mut self, text: &str) {
        let mut first_line = true;
        for line in text.lines() {
            if !first_line {
                self.add_newline();
            }
            first_line = false;
            self.output.push_str(line);
        }
        if ends_with_newline(text) {
            self.add_newline();
        }
    }

    // Add escaped text to the output. Newlines and punctuation are preserved.
    pub fn add_text(&mut self, text: &str) {
        for c in NewlineChars::new(text) {
            match c {
                c if c.general_category_group() == GeneralCategoryGroup::Punctuation => {
                    self.output.push('\\');
                    self.output.push(c);
                }
                '\n' => {
                    self.output.push('\\');
                    self.add_newline();
                }
                c => self.output.push(c),
            }
        }
    }

    fn indent<'s>(&'s mut self, initial: &str, indent: &'s str) -> StringBuilder<'s> {
        self.output.push_str(initial);
        StringBuilder {
            state: self.state.add_prefix(indent),
            output: self.output,
        }
    }

    fn borrow(&mut self) -> StringBuilder {
        StringBuilder {
            state: self.state,
            output: self.output,
        }
    }
}

struct MarkdownBuilder {
    output: String,
}

impl MarkdownBuilder {
    pub fn new() -> Self {
        MarkdownBuilder {
            output: String::new(),
        }
    }
}

struct Section<'a> {
    level: usize,
    output: &'a mut String,
}

impl<'a> Section<'a> {
    fn add_content(&mut self) -> Content {
        Content {
            next_separator: "",
            writer: StringBuilder::with_mut_string(self.output),
        }
    }

    fn into_section_builder(self) -> SubSectionList<'a> {
        SubSectionList {
            level: self.level,
            output: self.output,
        }
    }
}

#[derive(Clone, Copy)]
enum IndentState<'a> {
    Top,
    Indent {
        parent: &'a IndentState<'a>,
        newline_prefix: &'a str,
    },
}

impl<'a> IndentState<'a> {
    pub fn append_prefix(&self, output: &mut String) {
        match self {
            IndentState::Top => {}
            IndentState::Indent {
                parent,
                newline_prefix,
            } => {
                parent.append_prefix(output);
                output.push_str(newline_prefix);
            }
        }
    }

    pub fn add_prefix<'s: 'a>(&'s self, prefix: &'s str) -> IndentState<'s> {
        Self::Indent {
            parent: self,
            newline_prefix: prefix,
        }
    }
}
struct Content<'a> {
    writer: StringBuilder<'a>,
    next_separator: &'a str,
}

impl Content<'_> {
    fn add_paragraph(&mut self) -> Text {
        self.writer.add_raw_text(self.next_separator);
        self.next_separator = "\n";
        Text {
            curr_style: TextStyle::default(),
            writer: self.writer.borrow(),
        }
    }

    fn block_quote(&mut self) -> Text {
        self.writer.add_raw_text(self.next_separator);
        self.next_separator = "";
        Text {
            curr_style: TextStyle::default(),
            writer: self.writer.indent("> ", "> "),
        }
    }

    fn add_list(&mut self) -> List {
        self.writer.add_raw_text(self.next_separator);
        self.next_separator = "";
        List {
            writer: self.writer.borrow(),
        }
    }
}

enum StyleChange {
    On,
    Off,
    Unchanged,
}

impl StyleChange {
    pub fn from_state(init_state: bool, end_state: bool) -> Self {
        if init_state != end_state {
            if end_state {
                StyleChange::On
            } else {
                StyleChange::Off
            }
        } else {
            StyleChange::Unchanged
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
}

impl TextStyle {
    fn change(&self, new_style: TextStyle) -> TextStyleChange {
        TextStyleChange {
            bold: StyleChange::from_state(self.bold, new_style.bold),
            italic: StyleChange::from_state(self.italic, new_style.italic),
        }
    }
}

struct TextStyleChange {
    bold: StyleChange,
    italic: StyleChange,
}

struct Text<'a> {
    curr_style: TextStyle,
    writer: StringBuilder<'a>,
}

impl Text<'_> {
    fn add_text(&mut self, text: &str, style: TextStyle) -> &mut Self {
        if text.is_empty() {
            return self;
        }

        let change = self.curr_style.change(style);
        if let StyleChange::Off = change.bold {
            self.writer.add_raw_text("</b>");
        }
        if let StyleChange::Off = change.italic {
            self.writer.add_raw_text("</i>");
        }
        if let StyleChange::On = change.italic {
            self.writer.add_raw_text("<i>");
        }
        if let StyleChange::On = change.bold {
            self.writer.add_raw_text("<b>");
        }
        self.writer.add_text(text);
        self
    }
}

impl Drop for Text<'_> {
    fn drop(&mut self) {
        if self.curr_style.bold {
            self.writer.add_raw_text("</b>");
        }
        if self.curr_style.italic {
            self.writer.add_raw_text("</i>");
        }
        self.writer.add_newline();
    }
}

struct List<'a> {
    writer: StringBuilder<'a>,
}

impl List<'_> {
    fn add_item(&mut self) -> Content {
        todo!()
        // Content {
        //     writer: self.writer.indent("- ", "  "),
        // }
    }
}

struct SubSectionList<'a> {
    level: usize,
    output: &'a mut String,
}

impl SubSectionList<'_> {
    fn add_subsection(&mut self) -> Section {
        self.output
            .push_str(&format!("{} ", "#".repeat(self.level)));
        Section {
            level: self.level + 1,
            output: self.output,
        }
    }
}
