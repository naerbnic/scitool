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
            writeln!(f, "  {}", line)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Multiple(Vec<ValidationError>);

impl std::fmt::Display for Multiple {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for error in &self.0 {
            let contents = format!("{}", error);
            let mut first = true;
            for line in contents.lines() {
                if first {
                    writeln!(f, "- {}", line)?;
                    first = false;
                } else {
                    writeln!(f, "  {}", line)?;
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

pub trait IteratorExt: Iterator {
    #[expect(dead_code)]
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
        let mut errors = Vec::new();
        for item in self {
            if let Err(err) = validator(item) {
                match ValidationError::from_any(err) {
                    err @ (ValidationError::Single(_) | ValidationError::Context(_)) => {
                        errors.push(err)
                    }
                    ValidationError::Multiple(errs) => errors.extend(errs.0),
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else if errors.len() == 1 {
            Err(errors.pop().unwrap())
        } else {
            Err(ValidationError::Multiple(Multiple(errors)))
        }
    }

    fn validate_all_values<'a, K, V, F, E>(self, validator: F) -> Result<(), ValidationError>
    where
        Self: Iterator<Item = (&'a K, &'a V)>,
        K: 'a + std::fmt::Debug,
        V: 'a,
        F: Fn(&V) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        let mut errors = Vec::new();
        for (key, value) in self {
            if let Err(err) = validator(value) {
                let base_error = ValidationError::from_any(err);
                errors.push(ValidationError::Context(Context {
                    context: format!("{:?}", key),
                    error: Box::new(base_error),
                }));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else if errors.len() == 1 {
            Err(errors.pop().unwrap())
        } else {
            Err(ValidationError::Multiple(Multiple(errors)))
        }
    }
}

#[derive(Default)]
pub struct MultiValidator {
    errors: Vec<ValidationError>,
}

impl MultiValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn validate_ctxt<T, F, E>(
        &mut self,
        ctxt: impl Into<String>,
        item: &T,
        validator: F,
    ) -> &mut Self
    where
        F: FnOnce(&T) -> Result<(), E>,
        E: std::error::Error + Send + Sync + 'static,
    {
        if let Err(err) = validator(item) {
            self.errors.push(ValidationError::Context(Context {
                context: ctxt.into(),
                error: Box::new(ValidationError::from_any(err)),
            }))
        }
        self
    }

    pub fn build(&mut self) -> Result<(), ValidationError> {
        let mut errors = std::mem::take(&mut self.errors);
        if errors.is_empty() {
            Ok(())
        } else if errors.len() == 1 {
            Err(errors.pop().unwrap())
        } else {
            Err(ValidationError::Multiple(Multiple(errors)))
        }
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
