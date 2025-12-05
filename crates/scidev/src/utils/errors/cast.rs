use crate::utils::errors::{BoxError, ErrWrapper, register_wrapper, resolve_error};

pub(crate) struct CastChain<WrapE, E> {
    wrap: WrapE,
    _phantom: std::marker::PhantomData<E>,
}

impl<WrapE, E> CastChain<WrapE, E>
where
    WrapE: super::ErrWrapper,
    E: std::error::Error + Send + Sync + 'static,
{
    pub(crate) fn new(wrap: WrapE) -> Self {
        register_wrapper::<WrapE>();

        CastChain {
            wrap,
            _phantom: std::marker::PhantomData,
        }
    }

    pub(crate) fn with_cast<E2>(self, map: impl FnOnce(E2) -> E) -> BoxedCastChain<E>
    where
        E2: std::error::Error + Send + Sync + 'static,
    {
        self.resolve_registry().with_cast::<E2>(map)
    }

    fn resolve_registry(self) -> BoxedCastChain<E> {
        let CastChain { wrap, .. } = self;
        let wrap = resolve_error(Box::new(wrap));
        BoxedCastChain {
            state: BoxedCastChainState::HasWrap(wrap),
        }
    }
}

enum BoxedCastChainState<E> {
    HasWrap(BoxError),
    ResolvedError(E),
}

pub(crate) struct BoxedCastChain<E> {
    state: BoxedCastChainState<E>,
}

impl<E> BoxedCastChain<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    pub(crate) fn new(wrap: BoxError) -> Self {
        BoxedCastChain {
            state: BoxedCastChainState::HasWrap(wrap),
        }
    }
    pub(crate) fn with_cast<E2>(mut self, map: impl FnOnce(E2) -> E) -> Self
    where
        E2: std::error::Error + Send + Sync + 'static,
    {
        match self.state {
            BoxedCastChainState::HasWrap(other) => {
                self.state = match other.downcast() {
                    Ok(err) => BoxedCastChainState::ResolvedError(map(*err)),
                    Err(wrap) => BoxedCastChainState::HasWrap(wrap),
                }
            }
            BoxedCastChainState::ResolvedError(_) => {}
        }
        self
    }

    pub(crate) fn finish<WrapErr2>(self, map: impl FnOnce(WrapErr2) -> E) -> E
    where
        WrapErr2: ErrWrapper,
    {
        match self.state {
            BoxedCastChainState::HasWrap(wrap) => map(WrapErr2::wrap_box(wrap)),
            BoxedCastChainState::ResolvedError(err) => err,
        }
    }

    pub(crate) fn finish_box(self, map: impl FnOnce(BoxError) -> E) -> E {
        match self.state {
            BoxedCastChainState::HasWrap(wrap) => map(wrap),
            BoxedCastChainState::ResolvedError(err) => err,
        }
    }
}
