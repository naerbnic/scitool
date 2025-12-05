use std::{any::Any, backtrace::Backtrace};

use crate::utils::errors::BoxError;

trait Displayable: std::fmt::Display + std::fmt::Debug {}

impl<T> Displayable for T where T: std::fmt::Display + std::fmt::Debug {}

type BoxDisplay = Box<dyn Displayable + Send + Sync + 'static>;

pub struct OpaqueError {
    context: Vec<BoxDisplay>,
    backtrace: Backtrace,
    source: Option<BoxError>,
}

impl OpaqueError {
    fn from_boxed(err: BoxError) -> Self {
        match err.downcast::<Self>() {
            Ok(opaque) => *opaque,
            Err(err) => Self {
                context: Vec::new(),
                backtrace: Backtrace::capture(),
                source: Some(err),
            },
        }
    }

    fn try_cast<T: Any + Send + Sync + 'static>(value: T) -> Result<Self, T> {
        if std::any::TypeId::of::<T>() == std::any::TypeId::of::<OpaqueError>() {
            let box_any: Box<dyn Any + Send + Sync> = Box::new(value);
            Ok(box_any
                .downcast::<OpaqueError>()
                .map(|boxed_opaque| *boxed_opaque)
                .expect("TypeId check failed"))
        } else {
            Err(value)
        }
    }

    pub fn new<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::from_boxed(Box::new(err))
    }

    pub fn msg<M>(msg: M) -> Self
    where
        M: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
    {
        match Self::try_cast(msg) {
            Ok(opaque) => opaque,
            Err(msg) => Self {
                context: vec![Box::new(msg)],
                backtrace: Backtrace::capture(),
                source: None,
            },
        }
    }

    pub fn with_context<M, E>(err: E, msg: M) -> Self
    where
        M: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
        E: std::error::Error + Send + Sync + 'static,
    {
        match Self::try_cast(err) {
            Ok(mut opaque) => {
                opaque.context.push(Box::new(msg));
                opaque
            }
            Err(err) => Self {
                context: vec![Box::new(msg)],
                backtrace: Backtrace::capture(),
                source: Some(Box::new(err)),
            },
        }
    }
}

impl std::error::Error for OpaqueError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Some(boxed_err) = &self.source {
            Some(&**boxed_err)
        } else {
            None
        }
    }
}

impl From<BoxError> for OpaqueError {
    fn from(err: BoxError) -> Self {
        OpaqueError::from_boxed(err)
    }
}

impl std::fmt::Display for OpaqueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            if let Some((primary, context)) = self.context.split_last() {
                writeln!(f, "{primary:#}")?;
                for context in context.iter().rev() {
                    writeln!(f, "Caused by: {context}")?;
                }

                if let Some(source) = &self.source {
                    writeln!(f, "Source Error:\n{source}")?;
                }
                return Ok(());
            }

            let source = self
                .source
                .as_ref()
                .expect("OpaqueError must have context or source");

            // Print just the primary source error.
            writeln!(f, "{source:#}")?;
        }

        if let Some(last) = self.context.last() {
            return write!(f, "{last}");
        }

        let source = self
            .source
            .as_ref()
            .expect("OpaqueError must have context or source");
        write!(f, "{source}")
    }
}

impl std::fmt::Debug for OpaqueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            return f
                .debug_struct("OpaqueError")
                .field("context", &self.context)
                .field("backtrace", &self.backtrace)
                .field("source", &self.source)
                .finish();
        }

        if let Some((primary, context)) = self.context.split_last() {
            writeln!(f, "{primary:#}")?;
            for context in context.iter().rev() {
                writeln!(f, "Caused by: {context:#}")?;
            }
            if self.backtrace.status() == std::backtrace::BacktraceStatus::Captured {
                writeln!(f, "Backtrace:\n{:#}", self.backtrace)?;
            }

            if let Some(source) = &self.source {
                writeln!(f, "Source Error:\n{source:?}")?;
            }
            return Ok(());
        }

        let source = self
            .source
            .as_ref()
            .expect("OpaqueError must have context or source");

        // Print just the primary source error.
        writeln!(f, "{source}")?;

        if self.backtrace.status() == std::backtrace::BacktraceStatus::Captured {
            writeln!(f, "Backtrace:\n{:#}", self.backtrace)?;
        }

        // Print the full source error, including possibly its own context,
        // backtrace, and so on.
        writeln!(f, "Full Source:\n{source:?}")
    }
}
