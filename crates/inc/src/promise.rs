#![expect(unsafe_code)]

mod core;
mod owned;
mod shared;

use std::{collections::VecDeque, pin::pin, sync::Arc};

use asynx::stream::Stream;
use asynx::stream::StreamExt;

use crate::{event::Event, executor::Executor};

pub(crate) use owned::{Fulfiller, Promise};
pub(crate) use shared::{SharedAnyPromise, SharedFulfiller, SharedPromise};

pub(crate) struct StreamFulfiller<T> {
    event_queue: VecDeque<Option<Arc<Event>>>,
    fulfiller: SharedFulfiller<Option<(T, PromiseStream<T>)>>,
}

impl<T> StreamFulfiller<T>
where
    T: Send + Sync + 'static,
{
    fn push_next(&mut self, value: T) -> Option<Arc<Event>> {
        let (promise, mut fulfiller) = SharedPromise::new();
        let event = Arc::new(Event::new());

        let next_stream = PromiseStream {
            event: event.clone(),
            promise,
        };
        std::mem::swap(&mut self.fulfiller, &mut fulfiller);

        fulfiller.fulfill_arc(Arc::new(Some((value, next_stream))));

        self.event_queue.push_back(Some(event));
        self.event_queue.pop_front().unwrap()
    }

    pub fn send_sync(&mut self, value: T) {
        if let Some(next_event) = self.push_next(value) {
            next_event.wait_sync();
        }
    }

    pub async fn send_async(&mut self, value: T) {
        if let Some(next_event) = self.push_next(value) {
            next_event.wait_async().await;
        }
    }

    pub fn finish(self) {
        self.fulfiller.fulfill_arc(Arc::new(None));
    }
}

pub struct PromiseStream<T> {
    event: Arc<Event>,
    promise: SharedPromise<Option<(T, PromiseStream<T>)>>,
}

impl<T> PromiseStream<T>
where
    T: Send + Sync + 'static,
{
    fn new() -> (Self, StreamFulfiller<T>) {
        let initial_event = Arc::new(Event::new());

        // This initial queue skips the first slot, to allow the fulfiller to
        // create one value ahead of the consumer.
        let event_queue = VecDeque::from([None, Some(initial_event.clone())]);

        let (promise, fulfiller) = SharedPromise::new();
        let stream_fulfiller = StreamFulfiller {
            event_queue,
            fulfiller,
        };

        (
            Self {
                event: initial_event,
                promise,
            },
            stream_fulfiller,
        )
    }

    pub(crate) fn from_iter<It>(executor: &Executor, iter: It) -> PromiseStream<T>
    where
        It: Iterator<Item = T> + Send + 'static,
    {
        // Use a synchronous semaphore to block until each item in the
        // iterator has been consumed, as opposed to pumping all of the
        // values at once.
        let (stream, mut fulfiller) = PromiseStream::new();
        executor.schedule(move || {
            for value in iter {
                fulfiller.send_sync(value);
            }
            fulfiller.finish();
        });
        stream
    }

    pub(crate) fn from_stream<S>(executor: &Executor, iter: S) -> PromiseStream<T>
    where
        S: Stream<Item = T> + Send + 'static,
    {
        // Use a synchronous semaphore to block until each item in the
        // iterator has been consumed, as opposed to pumping all of the
        // values at once.
        let (stream, mut fulfiller) = PromiseStream::new();
        executor.schedule_async(async move {
            let mut iter = pin!(iter);
            while let Some(value) = iter.next().await {
                fulfiller.send_async(value).await;
            }
            fulfiller.finish();
        });
        stream
    }

    pub fn next(&self) -> SharedPromise<Option<(T, PromiseStream<T>)>> {
        self.event.signal();
        self.promise.clone()
    }
}

impl<T> Clone for PromiseStream<T> {
    fn clone(&self) -> Self {
        Self {
            event: self.event.clone(),
            promise: self.promise.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_promise_stream() {
        let executor = Executor::new_simple();
        let values = vec![1i32, 2, 3];
        let mut stream = PromiseStream::from_iter(&executor, values.into_iter());

        let mut result = Vec::new();
        while let Some((value, next)) = stream.next().get_sync() {
            result.push(*value);
            stream = next.clone();
        }

        assert_eq!(result, vec![1i32, 2, 3]);
    }
}
