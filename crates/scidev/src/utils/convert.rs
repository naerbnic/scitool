use std::{any::TypeId, mem::MaybeUninit};

pub(crate) fn convert_if_different<T, Target, F>(value: T, convert: F) -> Target
where
    T: 'static,
    Target: 'static,
    F: FnOnce(T) -> Target,
{
    if TypeId::of::<T>() == TypeId::of::<Target>() {
        // SAFETY: We just checked that T and Target are the same type.
        let mut value = MaybeUninit::new(value);
        #[allow(unsafe_code)]
        unsafe {
            value.as_mut_ptr().cast::<Target>().read()
        }
    } else {
        convert(value)
    }
}
