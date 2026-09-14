//! Crate-level re-exports for task macros.

/// Internal crate-level re-exports.
#[doc(hidden)]
pub mod __runtime {
    pub use scidev_task_impl::runtime::*;
}

pub mod macros {
    pub use scidev_task_macros::*;
}

#[cfg(test)]
mod tests {
    use crate::macros::task_struct;

    #[task_struct]
    mod foo {
        #[derive(Clone)]
        pub(crate) struct Foo {
            input: i32,
        }

        impl Foo {
            pub(super) fn slot_a(&self) -> i32 {
                self.input * 2
            }

            pub(super) fn slot_b(&self) -> i32 {
                *self.slot_a() - 3
            }

            pub(super) fn recursive_fails(&self) -> u32 {
                *self.recursive_fails()
            }

            #[result]
            pub(super) fn result_fn(&self) -> Result<i32, String> {
                Ok(self.input * 2)
            }
        }
    }

    #[test]
    fn basic_works() {
        let foo = foo::Foo::new(10);
        assert_eq!(*foo.slot_b(), 17);
        assert_eq!(*foo.slot_a(), 20);
    }

    #[test]
    #[should_panic(expected = "Reentrant access to task slot")]
    fn recursive_fails() {
        let foo = foo::Foo::new(10);
        let _ = *foo.recursive_fails();
    }

    #[test]
    fn test_result_fn_returns_cached_value() {
        let foo = foo::Foo::new(10);
        let result1 = foo.result_fn().unwrap();
        assert_eq!(*result1, 20);
        let result2 = foo.result_fn().unwrap();
        assert_eq!(*result2, 20);

        assert!(std::ptr::eq(result1, result2));
    }
}
