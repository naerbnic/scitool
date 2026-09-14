macro_rules! syn_bail {
    ([$span:expr] => $($body:tt)*) => {
        return Err(syn::Error::new($span, format!($($body)*)))
    };
    ($span:expr => $($body:tt)*) => {
        return Err(syn::Error::new_spanned($span, format!($($body)*)))
    };
}
pub(crate) use syn_bail;

macro_rules! syn_ensure {
    ($span:expr => $cond:expr; $($body:tt)*) => {
        if !$cond {
            $crate::prelude::syn_bail!($span => $($body)*);
        }
    };
}

pub(crate) use syn_ensure;
