use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::utils::errors::{BoxError, ErrWrapper, resolve_error};

pub(crate) struct ErrorCast<E> {
    cast_map: HashMap<TypeId, Box<dyn Fn(BoxError) -> E + Send + Sync>>,
    generic_cast: Box<dyn Fn(BoxError) -> E + Send + Sync>,
}

impl<E> ErrorCast<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    pub(crate) fn cast_err<E2>(&self, err: E2) -> E
    where
        E2: std::error::Error + Send + Sync + 'static,
    {
        self.cast_boxed(Box::new(err))
    }

    pub(crate) fn cast_boxed(&self, err: BoxError) -> E {
        let resolved_err = resolve_error(err);
        let type_id = Any::type_id(&*resolved_err);
        if let Some(cast_fn) = self.cast_map.get(&type_id) {
            return cast_fn(resolved_err);
        }
        (self.generic_cast)(resolved_err)
    }
}

pub(crate) struct Builder<E> {
    error_cast: ErrorCast<E>,
}

impl<E> Builder<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[expect(dead_code)]
    pub(crate) fn new<WrapErr>(generic_cast: impl Fn(WrapErr) -> E + Send + Sync + 'static) -> Self
    where
        WrapErr: ErrWrapper,
    {
        Builder {
            error_cast: ErrorCast {
                cast_map: HashMap::new(),
                generic_cast: Box::new(move |err| generic_cast(WrapErr::wrap_box(err))),
            },
        }
    }

    pub(crate) fn new_from<T>(generic_cast: impl Fn(T) -> E + Send + Sync + 'static) -> Self
    where
        T: From<BoxError> + 'static,
    {
        Builder {
            error_cast: ErrorCast {
                cast_map: HashMap::new(),
                generic_cast: Box::new(move |err| generic_cast(T::from(err))),
            },
        }
    }

    pub(crate) fn with_cast<E2>(mut self, map: impl Fn(E2) -> E + Send + Sync + 'static) -> Self
    where
        E2: std::error::Error + Send + Sync + 'static,
    {
        let cast_fn = move |err: BoxError| {
            let box_error: Box<E2> = err
                .downcast()
                .expect("Should only be called if the type matches");
            map(*box_error)
        };
        self.error_cast
            .cast_map
            .insert(TypeId::of::<E2>(), Box::new(cast_fn));
        self
    }

    pub(crate) fn build(self) -> ErrorCast<E> {
        self.error_cast
    }
}

pub(crate) trait ErrorCastable
where
    Self: Sized,
{
    fn error_cast() -> &'static ErrorCast<Self>;
}

macro_rules! impl_error_castable {
    ($err_ty:ty, $from_fn:path $(, $($cast:path),* $(,)?)?) => {
        impl $crate::utils::errors::ErrorCastable for $err_ty {
            fn error_cast() -> &'static $crate::utils::errors::ErrorCast<Self> {
                static CAST: std::sync::OnceLock<$crate::utils::errors::ErrorCast<$err_ty>> =
                    std::sync::OnceLock::new();
                CAST.get_or_init(|| $crate::utils::errors::ErrorCastBuilder::new_from($from_fn)
                    $($(.with_cast($cast))*)?
                    .build())
            }
        }

        impl From<BoxError> for $err_ty {
            fn from(err: BoxError) -> Self {
                <Self as $crate::utils::errors::ErrorCastable>::error_cast().cast_boxed(err)
            }
        }
    };
}

pub(crate) use impl_error_castable;
