use std::fmt::Display;

pub trait ContextAppendable {
    fn add_context<T>(self, context: T) -> Self
    where
        T: Display;
}

pub trait ResultExt<T, E> {
    #[must_use]
    fn with_ctxt<Ctxt>(self, context: Ctxt) -> Self
    where
        Ctxt: Display;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: ContextAppendable,
{
    fn with_ctxt<Ctxt>(self, context: Ctxt) -> Self
    where
        Ctxt: Display,
    {
        self.map_err(move |e| e.add_context(context))
    }
}
