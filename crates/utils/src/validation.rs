#[derive(Debug)]
pub struct Context {
    pub context: String,
    pub error: Box<ValidationError>,
}

impl std::fmt::Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.context)?;
        let contents = format!("{}", self.error);
        for line in contents.lines() {
            writeln!(f, "  {line}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Multiple(Vec<ValidationError>);

impl std::fmt::Display for Multiple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for error in &self.0 {
            let contents = format!("{error}");
            let mut first = true;
            for line in contents.lines() {
                if first {
                    writeln!(f, "- {line}")?;
                    first = false;
                } else {
                    writeln!(f, "  {line}")?;
                }
            }
        }
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ValidationError {
    #[error(transparent)]
    Single(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("{0}")]
    Multiple(Multiple),
    #[error("{0}")]
    Context(Context),
}

impl ValidationError {
    pub fn other<E>(err: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        ValidationError::Single(err.into())
    }

    pub fn from_boxed(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        match err.downcast::<ValidationError>() {
            Ok(err) => *err,
            Err(err) => ValidationError::other(err),
        }
    }

    pub fn from_any<E>(err: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::from_boxed(Box::new(err))
    }

    pub fn with_context(self, context: impl Into<String>) -> Self {
        match self {
            ValidationError::Context(mut ctxt) => {
                ctxt.context = format!("{}: {}", context.into(), ctxt.context);
                ValidationError::Context(ctxt)
            }
            _ => ValidationError::Context(Context {
                context: context.into(),
                error: Box::new(self),
            }),
        }
    }

    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (ValidationError::Multiple(mut first), ValidationError::Multiple(second)) => {
                first.0.extend(second.0);
                ValidationError::Multiple(first)
            }
            (ValidationError::Multiple(mut multi), single) => {
                multi.0.push(single);
                ValidationError::Multiple(multi)
            }
            (single, ValidationError::Multiple(mut multi)) => {
                multi.0.insert(0, single);
                ValidationError::Multiple(multi)
            }
            (first, second) => ValidationError::Multiple(Multiple(vec![first, second])),
        }
    }
}

impl From<String> for ValidationError {
    fn from(s: String) -> Self {
        ValidationError::Single(s.into())
    }
}

impl<T> From<Box<T>> for ValidationError
where
    T: std::error::Error + Send + Sync + 'static,
{
    fn from(err: Box<T>) -> Self {
        ValidationError::from_boxed(err)
    }
}

pub trait ResultExt {
    fn join(self, other: Self) -> Self;
    fn join_err(self, other: ValidationError) -> Self;
    fn append(&mut self, other: Self);
    fn append_err(&mut self, other: ValidationError);
    fn with_context(self, context: impl Into<String>) -> Self;
}

impl ResultExt for Result<(), ValidationError> {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(first), Ok(())) => Err(first),
            (Ok(()), Err(second)) => Err(second),
            (Err(first), Err(second)) => Err(first.join(second)),
        }
    }

    fn join_err(self, other: ValidationError) -> Self {
        match self {
            Ok(()) => Err(other),
            Err(err) => Err(err.join(other)),
        }
    }

    fn append(&mut self, other: Self) {
        *self = std::mem::replace(self, Ok(())).join(other);
    }

    fn append_err(&mut self, other: ValidationError) {
        *self = std::mem::replace(self, Ok(())).join_err(other);
    }

    fn with_context(self, context: impl Into<String>) -> Self {
        match self {
            Ok(()) => Ok(()),
            Err(err) => Err(err.with_context(context)),
        }
    }
}

pub trait IteratorExt: Iterator {
    fn validate_all<F, E>(self, validator: F) -> Result<(), ValidationError>
    where
        F: Fn(Self::Item) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static;

    fn validate_all_values<'a, K, V, F, E>(self, validator: F) -> Result<(), ValidationError>
    where
        Self: Iterator<Item = (&'a K, &'a V)>,
        K: 'a + std::fmt::Debug,
        V: 'a,
        F: Fn(&V) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static;
}

impl<I> IteratorExt for I
where
    I: Iterator,
{
    fn validate_all<F, E>(self, validator: F) -> Result<(), ValidationError>
    where
        F: Fn(Self::Item) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let mut result = Ok(());
        for item in self {
            result = result.join(validator(item).map_err(ValidationError::from_any));
        }
        result
    }

    fn validate_all_values<'a, K, V, F, E>(self, validator: F) -> Result<(), ValidationError>
    where
        Self: Iterator<Item = (&'a K, &'a V)>,
        K: 'a + std::fmt::Debug,
        V: 'a,
        F: Fn(&V) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let mut result = Ok(());
        for (key, value) in self {
            result = result
                .join(validator(value).map_err(|err| {
                    ValidationError::from_any(err).with_context(format!("{key:?}"))
                }));
        }
        result
    }
}

pub struct MultiValidator {
    result: Result<(), ValidationError>,
}

impl MultiValidator {
    pub fn new() -> Self {
        Self { result: Ok(()) }
    }

    pub fn with_result<E>(&mut self, item: Result<(), E>) -> &mut Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        if let Err(err) = item {
            self.result.append_err(ValidationError::from_any(err))
        }
        self
    }

    pub fn with_err<E>(&mut self, item: E) -> &mut Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        self.result.append_err(ValidationError::from_any(item));
        self
    }

    pub fn validate_ctxt<F, E>(&mut self, ctxt: impl Into<String>, validator: F) -> &mut Self
    where
        F: FnOnce() -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        self.result.append(
            validator()
                .map_err(ValidationError::from_any)
                .with_context(ctxt),
        );
        self
    }

    pub fn build(&mut self) -> Result<(), ValidationError> {
        std::mem::replace(&mut self.result, Ok(()))
    }
}

impl Default for MultiValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_trippable() {
        let err = ValidationError::Multiple(Multiple(vec![
            "test".to_string().into(),
            "test2".to_string().into(),
        ]));
        let err: Box<dyn std::error::Error + Send + Sync> = Box::new(err);
        let err = ValidationError::from_boxed(err);
        assert!(matches!(err, ValidationError::Multiple(_)));
    }

    #[test]
    fn test_auto_wrap() {
        let err: Box<dyn std::error::Error + Send + Sync> = "test".to_string().into();
        let err = ValidationError::from_boxed(err);
        assert!(matches!(err, ValidationError::Single(_)));
    }
}
