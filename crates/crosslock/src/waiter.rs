use std::sync::Arc;

mod inner {
    use std::{sync::Mutex, thread::Thread};

    #[derive(Debug)]
    enum State<T> {
        Available(Option<T>),
        Waiting(Option<Thread>),
    }

    #[derive(Debug)]
    pub(super) struct Inner<T>(Mutex<State<T>>);

    impl<T> Inner<T> {
        pub(super) fn new_waiting() -> Self {
            Self(Mutex::new(State::Waiting(None)))
        }

        pub(super) fn wait(&self) -> T {
            {
                let mut inner = self.0.lock().unwrap();
                let state = &mut *inner;
                let thread: &mut Option<Thread> = match state {
                    State::Available(value) => {
                        return value.take().expect("Only one call to wait is allowed");
                    }
                    State::Waiting(thread) => thread,
                };
                assert!(thread.is_none());
                *thread = Some(std::thread::current());
            }
            // We will now be unparked when the waker is notified. Park, checking
            // state in a loop to handle spurious wakeups.
            loop {
                std::thread::park();
                {
                    let mut inner = self.0.lock().unwrap();
                    if let State::Available(value) = &mut *inner {
                        return value.take().expect("Only one call to wait is allowed");
                    }
                }
            }
        }

        pub(super) fn wake(&self, value: T) {
            let mut inner = self.0.lock().unwrap();
            let state = &mut *inner;
            let State::Waiting(threads) = std::mem::replace(state, State::Available(Some(value)))
            else {
                panic!("Wake should only be called once");
            };

            // We want to unpark while holding on to the lock, to avoid a spurious wakeup race
            // between unlocking and unparking, a thread can be unparked without parking afterwads,
            // leading to an unintentional spurious wakeup on next park.
            if let Some(thread) = threads {
                thread.unpark();
            }
        }
    }
}

use inner::Inner;

pub(crate) struct Waiter<T> {
    inner: Arc<Inner<T>>,
}

impl<T> Waiter<T> {
    /// Creates a new Waiter and waker pair
    pub(crate) fn new() -> (Self, Waker<T>) {
        let inner = Arc::new(Inner::new_waiting());
        let waker = Waker {
            inner: Some(inner.clone()),
        };
        (Self { inner }, waker)
    }

    pub(crate) fn wait(&self) -> T {
        self.inner.wait()
    }
}

#[derive(Debug)]
pub(crate) struct Waker<T> {
    inner: Option<Arc<Inner<T>>>,
}

impl<T> Waker<T> {
    /// Notifies the associated Waiter, waking any thread waiting on it.
    ///
    /// If there isn't a thread waiting, all future calls to `wait` will
    /// return immediately until a `wait` call is made.
    pub(crate) fn wake(mut self, value: T) {
        self.inner.take().unwrap().wake(value);
    }
}

impl<T> Drop for Waker<T> {
    fn drop(&mut self) {
        assert!(
            self.inner.is_none(),
            "Waker must be explicitly woken before being dropped, at risk of deadlock"
        );
    }
}
