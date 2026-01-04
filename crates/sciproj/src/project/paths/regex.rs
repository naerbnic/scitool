use regex_automata::{
    meta::{BuildError, Config, Regex},
    util::captures::Captures,
};
use regex_syntax::hir::{self, Capture, Hir, Look, Repetition};

struct HirPrinter<'a>(&'a Hir);

impl std::fmt::Debug for HirPrinter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut printer = hir::print::Printer::new();
        printer.print(&self.0, f)
    }
}

// A struct representing a node in the regular expression tree.
//
// This implementation allows for checking of ambiguities on a particular value by switching the greediness of the regular expression.
#[derive(Clone)]
pub(super) struct Node {
    // We keep two versions of the regular expression to be able to detect
    // ambiguities on a particular value.
    greedy: Hir,
    lazy: Hir,
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("greedy", &HirPrinter(&self.greedy))
            .field("lazy", &HirPrinter(&self.lazy))
            .finish()
    }
}

impl Node {
    pub fn from_regex(regex: &str) -> Result<Self, regex_syntax::Error> {
        let mut parser_builder = regex_syntax::ParserBuilder::new();
        let mut greedy_parser = parser_builder.utf8(true).build();
        let mut lazy_parser = parser_builder.utf8(true).swap_greed(true).build();

        let greedy_hir = greedy_parser.parse(regex)?;
        let lazy_hir = lazy_parser.parse(regex)?;

        Ok(Self {
            greedy: greedy_hir,
            lazy: lazy_hir,
        })
    }

    pub fn empty() -> Self {
        Self {
            greedy: Hir::empty(),
            lazy: Hir::empty(),
        }
    }

    pub fn concat(nodes: impl IntoIterator<Item = Node>) -> Self {
        let (greedy, lazy) = nodes
            .into_iter()
            .map(|node| (node.greedy, node.lazy))
            .unzip();

        Self {
            greedy: Hir::concat(greedy),
            lazy: Hir::concat(lazy),
        }
    }

    pub fn alt(nodes: impl IntoIterator<Item = Node>) -> Self {
        let (greedy, lazy) = nodes
            .into_iter()
            .map(|node| (node.greedy, node.lazy))
            .unzip();

        Self {
            greedy: Hir::alternation(greedy),
            lazy: Hir::alternation(lazy),
        }
    }

    pub fn literal(str: impl Into<String>) -> Self {
        let bytes = str.into().into_bytes();
        Self {
            greedy: Hir::literal(bytes.clone()),
            lazy: Hir::literal(bytes),
        }
    }

    pub fn optional(self) -> Self {
        Self {
            greedy: Hir::repetition(Repetition {
                min: 0,
                max: Some(1),
                greedy: true,
                sub: Box::new(self.greedy),
            }),
            lazy: Hir::repetition(Repetition {
                min: 0,
                max: Some(1),
                greedy: false,
                sub: Box::new(self.lazy),
            }),
        }
    }

    pub fn capture(self, index: u32, capture_name: &str) -> Self {
        Self {
            greedy: Hir::capture(Capture {
                index,
                name: Some(capture_name.to_string().into_boxed_str()),
                sub: Box::new(self.greedy),
            }),
            lazy: Hir::capture(Capture {
                index,
                name: Some(capture_name.to_string().into_boxed_str()),
                sub: Box::new(self.lazy),
            }),
        }
    }

    pub fn build_matcher(self) -> Result<UnambiguousRegex, BuildError> {
        // We only provide full matchers, so insert Hir for the start and end of the input string.
        let greedy = Hir::concat(vec![
            Hir::look(Look::Start),
            self.greedy,
            Hir::look(Look::End),
        ]);
        let lazy = Hir::concat(vec![
            Hir::look(Look::Start),
            self.lazy,
            Hir::look(Look::End),
        ]);
        let mut builder = regex_automata::meta::Builder::new();
        builder.configure(Config::default().utf8_empty(true));
        let greedy = builder.build_from_hir(&greedy)?;
        let lazy = builder.build_from_hir(&lazy)?;

        Ok(UnambiguousRegex { greedy, lazy })
    }
}

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error("String matched ambiguously with the capture groups.")]
    AmbiguousMatch,
}

pub(super) struct UnambiguousRegex {
    greedy: Regex,
    lazy: Regex,
}

impl UnambiguousRegex {
    pub fn match_unambiguous(&self, text: &str) -> Result<Option<Captures>, Error> {
        let mut greedy_captures = self.greedy.create_captures();
        self.greedy.captures(text, &mut greedy_captures);
        let mut lazy_captures = self.lazy.create_captures();
        self.lazy.captures(text, &mut lazy_captures);

        if !greedy_captures.is_match() {
            assert!(
                !lazy_captures.is_match(),
                "Both regexes should either match or not match."
            );
            return Ok(None);
        }

        for (greedy_capture, lazy_capture) in greedy_captures.iter().zip(lazy_captures.iter()) {
            if greedy_capture != lazy_capture {
                return Err(Error::AmbiguousMatch);
            }
        }

        Ok(Some(greedy_captures))
    }
}
