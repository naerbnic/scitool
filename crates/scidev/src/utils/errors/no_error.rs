/// An error that can not be produced.
#[derive(Debug, Copy, Clone)]
pub struct NoError(std::convert::Infallible);

impl NoError {
    /// Return any value if you have a `NoError`.
    ///
    /// Since `NoError` can never be constructed, if you have one, you can produce
    /// any type of value you want.
    pub fn absurd(&self) -> ! {
        match self.0 {}
    }
}

impl std::fmt::Display for NoError {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {}
    }
}

impl std::error::Error for NoError {}
