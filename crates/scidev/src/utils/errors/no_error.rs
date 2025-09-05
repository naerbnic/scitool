

#[derive(Debug, Copy, Clone)]
pub struct NoError(std::convert::Infallible);

impl NoError {
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