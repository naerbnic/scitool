mod context;
mod other;

use std::error::Error as StdError;

pub mod prelude {
    pub use super::ErrorExt as _;
    pub use super::context::ResultExt as _;
    pub use super::other::OptionExt as _;
    pub use super::other::ResultExt as _;
}

use std::fmt::{Debug, Display};
use std::sync::Arc;

pub(crate) use other::{OtherError, OtherMapper, bail_other, ensure_other};

pub trait ErrorExt {
    fn get_in_chain<E: std::error::Error + 'static>(&self) -> Option<&E>;
}

impl<E> ErrorExt for E
where
    E: std::error::Error + 'static,
{
    fn get_in_chain<Target: std::error::Error + 'static>(&self) -> Option<&Target> {
        let mut current: &(dyn std::error::Error + 'static) = self;
        loop {
            if let Some(target) = current.downcast_ref::<Target>() {
                return Some(target);
            }
            match current.source() {
                Some(source) => current = source,
                None => return None,
            }
        }
    }
}

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
struct ScopeInfo {
    data_size: usize,
    scopes: Vec<ConcreteScopeItem>,
}

impl Display for ScopeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "- {} block", self.data_size)?;
        for scope in &self.scopes {
            writeln!(f, "\n- {scope}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct BlockContext<'a>(ContextInner<'a>);

impl BlockContext<'_> {
    /// Create a new root context.
    #[must_use]
    pub fn new_root(data_size: usize) -> Self {
        Self(ContextInner::Root { data_size })
    }

    /// Create a new nested context.
    pub fn nested<D>(&self, start: usize, end: usize, description: D) -> BlockContext<'_>
    where
        D: Display + Debug + Clone + 'static,
    {
        BlockContext(ContextInner::Nested {
            parent: &self.0,
            scope_item: ScopeItem::new(start, end, description),
        })
    }

    pub fn create_error<E>(&self, position: usize, message: E) -> InvalidDataError<E>
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        InvalidDataError {
            scope_info: self.0.make_scope_info(),
            position,
            message,
        }
    }
}

trait ContextLayer<'a>: std::fmt::Debug {
    fn make_scope_info(&self) -> ScopeInfo;
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
    fn make_scope_info(&self) -> ScopeInfo {
        match self {
            ContextInner::Root { data_size } => ScopeInfo {
                data_size: *data_size,
                scopes: Vec::new(),
            },
            ContextInner::Nested { parent, scope_item } => {
                let mut info = parent.make_scope_info();
                info.scopes.push(scope_item.to_concrete());
                info
            }
        }
    }
}

#[derive(Debug)]
pub struct InvalidDataError<E> {
    scope_info: ScopeInfo,
    position: usize,
    message: E,
}

impl<E> InvalidDataError<E> {
    pub fn map<F, R>(self, body: F) -> InvalidDataError<R>
    where
        F: FnOnce(E) -> R,
    {
        InvalidDataError {
            scope_info: self.scope_info,
            position: self.position,
            message: body(self.message),
        }
    }
}

impl<E: Display> Display for InvalidDataError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid data at position {}: {}{}",
            self.position, self.message, self.scope_info
        )
    }
}

impl<E: StdError + 'static> StdError for InvalidDataError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.message)
    }
}
