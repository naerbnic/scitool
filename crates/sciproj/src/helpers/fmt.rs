use std::fmt::{self, Write as _};

struct IndentWriter<'a, W> {
    writer: &'a mut W,
    indent: usize,
    at_line_start: bool,
}

impl<'a, W> IndentWriter<'a, W>
where
    W: fmt::Write,
{
    pub(crate) fn new(writer: &'a mut W, indent: usize) -> Self {
        Self {
            writer,
            indent,
            at_line_start: false,
        }
    }
}

impl<W> fmt::Write for IndentWriter<'_, W>
where
    W: fmt::Write,
{
    fn write_str(&mut self, mut s: &str) -> fmt::Result {
        while !s.is_empty() {
            // Find first part of s, up to after a newline.
            let to_write;
            let add_newline;
            if let Some((first, rest)) = s.split_once('\n') {
                to_write = first;
                s = rest;
                add_newline = true;
            } else {
                to_write = s;
                s = "";
                add_newline = false;
            }

            // If to_write is empty, that means that we had consecutive newlines.
            // There's no need to indent in that case.
            if self.at_line_start && !to_write.is_empty() {
                self.writer.write_str(&" ".repeat(self.indent))?;
                self.at_line_start = false;
            }

            self.writer.write_str(to_write)?;
            if add_newline {
                self.writer.write_str("\n")?;
                self.at_line_start = true;
            }
        }

        Ok(())
    }
}

/// A wrapper around a value that formats it following Debug default rules.
pub(crate) struct DebugIndent<'a, T>(&'a T)
where
    T: ?Sized;

impl<'a, T> DebugIndent<'a, T>
where
    T: ?Sized,
{
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn new(value: &'a T) -> Self {
        Self(value)
    }
}

impl<T> fmt::Debug for DebugIndent<'_, T>
where
    T: fmt::Debug + ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            // The default indent level for the alternate mode is 4.
            write!(IndentWriter::new(f, 4), "{:?}", self.0)
        } else {
            fmt::Debug::fmt(self.0, f)
        }
    }
}

pub(crate) struct Indent<'a, T>
where
    T: ?Sized,
{
    value: &'a T,
    indent: usize,
}

impl<'a, T> Indent<'a, T>
where
    T: ?Sized,
{
    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn new(value: &'a T) -> Self {
        Self { value, indent: 4 }
    }

    #[cfg_attr(not(test), expect(dead_code, reason = "experimental"))]
    pub(crate) fn indent(&self, indent: usize) -> Indent<'a, T> {
        Indent {
            value: self.value,
            indent,
        }
    }
}

impl<T> fmt::Debug for Indent<'_, T>
where
    T: fmt::Debug + ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(IndentWriter::new(f, self.indent), "{:?}", self.value)
    }
}

impl<T> fmt::Display for Indent<'_, T>
where
    T: fmt::Display + ?Sized,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(IndentWriter::new(f, self.indent), "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StringIndent<'a> {
        s: &'a str,
        indent: usize,
    }

    impl fmt::Display for StringIndent<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Display::fmt(&Indent::new(self.s).indent(self.indent), f)
        }
    }

    impl fmt::Debug for StringIndent<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Debug::fmt(&DebugIndent::new(self.s), f)
        }
    }

    #[test]
    fn test_non_indent() {
        // By default, the initial line is not indented.
        assert_eq!(
            format!(
                "{}",
                StringIndent {
                    s: "hello",
                    indent: 4
                }
            ),
            "hello"
        );
    }

    #[test]
    fn test_indent() {
        assert_eq!(
            format!(
                "{}",
                StringIndent {
                    s: "hello\nworld",
                    indent: 4
                }
            ),
            "hello\n    world"
        );
    }

    #[test]
    fn test_consecutive_newlines() {
        assert_eq!(
            format!(
                "{}",
                StringIndent {
                    s: "hello\n\nworld",
                    indent: 4
                }
            ),
            "hello\n\n    world"
        );
    }

    #[test]
    fn test_trailing_newline() {
        assert_eq!(
            format!(
                "{}",
                StringIndent {
                    s: "hello\nworld\n",
                    indent: 4
                }
            ),
            "hello\n    world\n"
        );
    }

    #[test]
    fn test_debug_indented() {
        assert_eq!(format!("{:?}", DebugIndent::new("hello")), "hello");
    }
}
