use crate::state::Event;
use tokio::sync::mpsc;

/// Async event receiver that wraps tokio mpsc
pub struct AsyncEventReceiver {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl AsyncEventReceiver {
    pub(crate) fn new(rx: mpsc::UnboundedReceiver<Event>) -> Self {
        Self { rx }
    }

    /// Create an empty receiver (used when the receiver is moved out)
    pub fn empty() -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self { rx }
    }

    /// Receive next event asynchronously
    pub async fn recv(&mut self) -> Option<Event> {
        self.rx.recv().await
    }

    /// Convert to a Stream for use with StreamExt
    pub fn into_stream(self) -> impl futures::Stream<Item = Event> {
        tokio_stream::wrappers::UnboundedReceiverStream::new(self.rx)
    }
}
