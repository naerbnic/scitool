use std::fmt::{Debug, Display};
use std::sync::Arc;

/// A trait representing a "displayable" item that will be referenced from
/// scopes.
trait Displayable: Display + Debug {}

impl<T> Displayable for T where T: Display + Debug {}

struct BoxDisplayable(Arc<dyn Displayable>);

impl BoxDisplayable {
    pub(crate) fn new<D>(disp: D) -> Self
    where
        D: Display + Debug + 'static,
    {
        Self(Arc::new(disp))
    }
}

impl std::fmt::Debug for BoxDisplayable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&*self.0, f)
    }
}

impl std::fmt::Display for BoxDisplayable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&*self.0, f)
    }
}

impl Clone for BoxDisplayable {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

/// A scope within a block of data.
#[derive(Debug, Clone)]
struct ScopeItem {
    start: usize,
    end: usize,
    description: Option<BoxDisplayable>,
}

impl ScopeItem {
    /// Create a new scope item.
    pub(crate) fn new<D>(start: usize, end: usize, description: D) -> Self
    where
        D: Display + Debug + Clone + 'static,
    {
        Self {
            start,
            end,
            description: Some(BoxDisplayable::new(description)),
        }
    }

    pub(crate) fn to_concrete(&self) -> ConcreteScopeItem {
        ConcreteScopeItem {
            start: self.start,
            end: self.end,
            description: self.description.as_ref().map(ToString::to_string),
        }
    }
}

/// A scope within a block of data.
#[derive(Debug, Clone)]
struct ConcreteScopeItem {
    start: usize,
    end: usize,
    description: Option<String>,
}

impl Display for ConcreteScopeItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(desc) = &self.description {
            write!(f, "[{}..{}] {}", self.start, self.end, desc)
        } else {
            write!(f, "[{}..{}]", self.start, self.end)
        }
    }
}

/// An error type that indicates that the data found in a block is invalid.
///
/// This should not represent an error in reading the data itself, only in the
/// format of the data.
#[derive(Debug)]
pub(crate) struct ScopeInfo {
    data_size: usize,
    scopes: Vec<ConcreteScopeItem>,
    position: usize,
}

impl Display for ScopeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "At byte position {}, ", self.position)?;
        for scope in &self.scopes {
            writeln!(f, "\n- In subblock in {scope}")?;
        }
        write!(f, "\n- In block of size {}", self.data_size)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BlockContext<'a>(ContextInner<'a>);

impl BlockContext<'_> {
    /// Create a new root context.
    #[must_use]
    pub(crate) fn new_root(data_size: usize) -> Self {
        Self(ContextInner::Root { data_size })
    }

    /// Create a new nested context.
    pub(crate) fn nested<D>(&self, start: usize, end: usize, description: D) -> BlockContext<'_>
    where
        D: Display + Debug + Clone + 'static,
    {
        BlockContext(ContextInner::Nested {
            parent: &self.0,
            scope_item: ScopeItem::new(start, end, description),
        })
    }

    pub(crate) fn make_context(&self, position: usize) -> ScopeInfo {
        let (data_size, scopes) = self.0.make_context();
        ScopeInfo {
            data_size,
            scopes,
            position,
        }
    }
}

trait ContextLayer<'a>: std::fmt::Debug {
    fn make_context(&self) -> (usize, Vec<ConcreteScopeItem>);
}

#[derive(Debug, Clone)]
enum ContextInner<'a> {
    Root {
        data_size: usize,
    },
    Nested {
        parent: &'a ContextInner<'a>,
        scope_item: ScopeItem,
    },
}

impl<'a> ContextLayer<'a> for ContextInner<'a> {
    #[track_caller]
    fn make_context(&self) -> (usize, Vec<ConcreteScopeItem>) {
        match self {
            ContextInner::Root { data_size } => (*data_size, Vec::new()),
            ContextInner::Nested { parent, scope_item } => {
                let (data_size, mut scopes) = parent.make_context();
                scopes.push(scope_item.to_concrete());
                (data_size, scopes)
            }
        }
    }
}
