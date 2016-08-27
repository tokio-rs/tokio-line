#![allow(dead_code)]

use futures::Poll;
use futures::stream::Stream;
use std::marker::PhantomData;

/// An empty stream
pub struct Empty<T, E> {
    marker: PhantomData<(T, E)>,
}

// TODO: Move this into futures-rs?
impl<T, E> Empty<T, E> {
    /// Create a new `Empty`
    pub fn new() -> Empty<T, E> {
        Empty { marker: PhantomData }
    }
}

impl<T, E> Stream for Empty<T, E>
    where T: Send + 'static,
          E: Send + 'static,
{
    type Item = T;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        Poll::Ok(None)
    }
}
