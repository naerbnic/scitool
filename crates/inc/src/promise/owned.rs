use std::task::{Context, Poll};

use crate::{
    event::wait_node::{EventToken, EventTrigger},
    executor::cancel::CancelledError,
    promise::core::SharedCell,
};

enum PromiseState<T> {
    Value(Option<T>),
    Cancelled,
}

pub(crate) struct Promise<T> {
    cell: SharedCell<PromiseState<T>>,
    ready_token: EventToken,
}

impl<T> Promise<T> {
    pub(crate) fn new() -> (Self, Fulfiller<T>) {
        let cell = SharedCell::new();
        let (ready_token, ready_trigger) = EventToken::new();
        // We are ready if we either cancel or are filled.
        (
            Self {
                cell: cell.clone(),
                ready_token,
            },
            Fulfiller {
                cell,
                trigger: ready_trigger,
            },
        )
    }

    pub(crate) fn new_ready(value: T) -> Self
    where
        T: Send + Sync + 'static,
    {
        let cell = SharedCell::with_value(PromiseState::Value(Some(value)));
        let ready_token = EventToken::new_triggered();
        Self { cell, ready_token }
    }

    fn try_take(&mut self) -> Option<Result<T, CancelledError>> {
        self.cell.with_opt(|state| {
            match state {
                PromiseState::Value(v) => {
                    // We panic if try_take succeeds twice.
                    Ok(std::mem::take(v).unwrap())
                }
                PromiseState::Cancelled => Err(CancelledError),
            }
        })
    }

    pub(crate) fn poll_take(&mut self, cx: &mut Context<'_>) -> Poll<Result<T, CancelledError>> {
        let token_result = std::task::ready!(self.ready_token.poll_triggered(cx));
        if let Ok(()) = token_result {
            self.cell
                .with_opt(|state| {
                    let PromiseState::Value(v) = state else {
                        panic!("No value after triggered");
                    };
                    Poll::Ready(Ok(std::mem::take(v).unwrap()))
                })
                .unwrap()
        } else {
            drop(self.cell.set_if_empty(PromiseState::Cancelled));
            Poll::Ready(Err(CancelledError))
        }
    }

    pub(crate) fn take_sync(&mut self) -> Result<T, CancelledError> {
        asynx::flavor::assert_in_sync();
        asynx::executor::block_on(async move { self.take_async().await })
    }

    pub(crate) async fn take_async(&mut self) -> Result<T, CancelledError> {
        asynx::flavor::assert_in_async();
        asynx::future::poll_fn(|cx| self.poll_take(cx)).await
    }
}

pub(crate) struct Fulfiller<T> {
    cell: SharedCell<PromiseState<T>>,
    trigger: EventTrigger,
}

impl<T> Fulfiller<T> {
    pub(crate) fn fulfill(self, value: T) -> Result<(), CancelledError> {
        if self
            .cell
            .set_if_empty(PromiseState::Value(Some(value)))
            .is_err()
        {
            // Already set.
            return Ok(());
        }

        self.trigger.trigger().map_err(|_| CancelledError)
    }
}
