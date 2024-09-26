use super::text::RichText;

mod builder;

// pub struct MarkdownDocument {
//     output: MarkdownBuilder,
// }

// impl MarkdownDocument {
//     pub fn new(title: RichText) -> Self {
//         let mut output = MarkdownBuilder::new();
//         output.write_section_text(/*first_line_prefix=*/ "# ", "# ", &title);
//         Self { output }
//     }
// }

// impl super::DocumentBuilder for MarkdownDocument {
//     fn add_chapter(&mut self, title: super::text::RichText) -> impl super::SectionBuilder {
//         self.output.write_single_line("## ", &title);
//         Section {
//             level: 3,
//             output: &mut self.output,
//         }
//     }
// }

// struct Section<'a> {
//     level: usize,
//     output: &'a mut MarkdownBuilder,
// }

// impl<'a> super::SectionBuilder for Section<'a> {
//     fn add_content(&mut self) -> impl super::ContentBuilder {
//         Content {
//             indent_level: 0,
//             output: self.output,
//         }
//     }

//     fn into_section_builder(self) -> impl super::SubSectionBuilder {
//         self
//     }
// }

// impl<'a> super::SubSectionBuilder for Section<'a> {
//     fn add_subsection(&mut self, title: RichText) -> impl super::SectionBuilder {
//         let prefix = format!("{} ", "#".repeat(self.level));
//         self.output.write_single_line(&prefix, &title);
//         Section {
//             level: self.level + 1,
//             output: self.output,
//         }
//     }
// }

// struct Content<'a> {
//     indent_level: usize,
//     output: &'a mut MarkdownBuilder,
// }

// impl<'a> super::ContentBuilder for Content<'a> {
//     fn add_paragraph(&mut self, text: RichText) {
//         self.output.write_section_text(
//             /*first_line_prefix=*/ &"  ".repeat(self.indent_level),
//             /*rest_line_prefix=*/ &"  ".repeat(self.indent_level),
//             &text,
//         );
//     }

//     fn add_list(&mut self) -> impl super::ListBuilder {
//         todo!()
//     }

//     fn add_dialogue(&mut self) -> impl super::DialogueBuilder {
//         todo!()
//     }
// }
