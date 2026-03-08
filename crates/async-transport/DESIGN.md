# transport2 - Async DAP Transport Crate Design

## Overview

The `transport2` crate provides an async transport layer for the Debug Adapter Protocol (DAP) using tokio and the tokio-util codec pattern. It focuses exclusively on the transport concern: encoding/decoding DAP messages over byte streams.

## Design Principles

1. **Single Responsibility**: Only handles transport - no business logic, no request tracking, no event routing
2. **Async-first**: Built on tokio with proper async I/O patterns
3. **Composable**: Provides (reader, writer) split for upstream multiplexing
4. **Testable**: Generic over transport to support in-memory testing
5. **Zero-copy where possible**: Uses `bytes::BytesMut` for buffer management

## Non-Goals (Delegated to Upstream Crates)

- Request-response correlation (RequestStore pattern) - belongs in debugger layer
- Event routing/channels - belongs in debugger layer
- Background polling threads - replaced by async tasks in consumers
- Connection management/retry logic - belongs in server/debugger layer
- Sequence number generation - belongs in client logic layer

## Architecture

```
                    transport2 crate scope
    +--------------------------------------------------+
    |                                                  |
    |   AsyncRead/AsyncWrite (TcpStream, etc.)         |
    |                    |                             |
    |                    v                             |
    |            +---------------+                     |
    |            |   DapCodec    |                     |
    |            | (Encoder +    |                     |
    |            |  Decoder)     |                     |
    |            +---------------+                     |
    |                    |                             |
    |                    v                             |
    |   +---------------------------------+            |
    |   |  Framed<T, DapCodec>           |            |
    |   |  (Stream<Message> + Sink<...>) |            |
    |   +---------------------------------+            |
    |           |                |                     |
    |           v                v                     |
    |   +-------------+  +-------------+               |
    |   | DapReader   |  | DapWriter   |               |
    |   | (Stream)    |  | (Sink)      |               |
    |   +-------------+  +-------------+               |
    |                                                  |
    +--------------------------------------------------+
                           |
                           v
              Upstream crate (debugger, etc.)
              - Request/response correlation
              - Event handling
              - Business logic
```

## Module Structure

```
crates/transport2/
  Cargo.toml
  src/
    lib.rs              # Public API exports
    codec.rs            # DapCodec (Encoder + Decoder impl)
    error.rs            # Transport-specific errors
    message.rs          # Re-export or define Message type
    transport.rs        # DapTransport trait and split functionality
    reader.rs           # DapReader wrapper (typed Stream)
    writer.rs           # DapWriter wrapper (typed Sink)

    # Testing utilities
    testing/
      mod.rs
      memory.rs         # In-memory transport for tests
```

## Key Types

### DapCodec

Implements both `tokio_util::codec::Encoder` and `Decoder` for DAP messages.

```rust
pub struct DapCodec {
    // Optional: max message size for DoS protection
    max_message_size: Option<usize>,
}

impl Decoder for DapCodec {
    type Item = Message;
    type Error = CodecError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error>;
}

impl Encoder<OutgoingMessage> for DapCodec {
    type Error = CodecError;

    fn encode(&mut self, item: OutgoingMessage, dst: &mut BytesMut) -> Result<(), Self::Error>;
}
```

### Message Types

Re-use existing types from transport crate or define compatible ones:

```rust
// Incoming messages (from debug adapter)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Message {
    Event(Event),
    Response(Response),
    Request(Request),  // Reverse requests from adapter
}

// Outgoing messages (to debug adapter)
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OutgoingMessage {
    Request(Request),
}
```

### DapTransport Trait

Generic abstraction over async transports:

```rust
pub trait DapTransport {
    type Read: AsyncRead + Unpin + Send;
    type Write: AsyncWrite + Unpin + Send;

    fn split(self) -> (Self::Read, Self::Write);
}

// Blanket impl for TcpStream
impl DapTransport for TcpStream {
    type Read = OwnedReadHalf;
    type Write = OwnedWriteHalf;

    fn split(self) -> (Self::Read, Self::Write) {
        TcpStream::into_split(self)
    }
}
```

### Reader and Writer Types

Typed wrappers around framed streams:

```rust
/// Async stream of incoming DAP messages
pub struct DapReader<R> {
    inner: FramedRead<R, DapCodec>,
}

impl<R: AsyncRead + Unpin> Stream for DapReader<R> {
    type Item = Result<Message, CodecError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<Self::Item>>;
}

/// Async sink for outgoing DAP messages
pub struct DapWriter<W> {
    inner: FramedWrite<W, DapCodec>,
}

impl<W: AsyncWrite + Unpin> DapWriter<W> {
    pub async fn send(&mut self, msg: OutgoingMessage) -> Result<(), CodecError>;
}
```

### Main Entry Point

```rust
/// Split a transport into reader/writer pair
pub fn split<T: DapTransport>(transport: T) -> (DapReader<T::Read>, DapWriter<T::Write>) {
    let (read, write) = transport.split();
    (
        DapReader::new(read),
        DapWriter::new(write),
    )
}

// Convenience for common case
pub async fn connect(addr: impl ToSocketAddrs) -> io::Result<(DapReader<OwnedReadHalf>, DapWriter<OwnedWriteHalf>)> {
    let stream = TcpStream::connect(addr).await?;
    Ok(split(stream))
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid UTF-8 in header")]
    InvalidUtf8,

    #[error("malformed Content-Length header")]
    MalformedHeader,

    #[error("missing Content-Length header")]
    MissingContentLength,

    #[error("content length {0} exceeds maximum {1}")]
    MessageTooLarge(usize, usize),

    #[error("JSON deserialization failed: {0}")]
    JsonDeserialize(#[from] serde_json::Error),

    #[error("JSON serialization failed: {0}")]
    JsonSerialize(serde_json::Error),
}
```

## Wire Protocol Implementation

DAP uses a simple Content-Length header protocol:

```
Content-Length: <length>\r\n
\r\n
<JSON body>
```

### Decoder Logic

1. Search for `\r\n\r\n` (header/body separator)
2. If not found, return `Ok(None)` (need more data)
3. Parse `Content-Length: <n>` from header section
4. Check if buffer has `header_len + 4 + content_length` bytes
5. If not, return `Ok(None)` (need more data)
6. Deserialize JSON body into `Message`
7. Advance buffer past consumed bytes
8. Return `Ok(Some(message))`

### Encoder Logic

1. Serialize message to JSON
2. Write `Content-Length: {len}\r\n\r\n`
3. Write JSON bytes

## Testing Infrastructure

### In-Memory Transport

```rust
pub struct MemoryTransport {
    read: tokio::io::DuplexStream,
    write: tokio::io::DuplexStream,
}

impl MemoryTransport {
    /// Create a connected pair of transports for testing
    pub fn pair() -> (Self, Self);
}
```

### Test Helpers

```rust
pub mod testing {
    /// Create a reader from raw bytes
    pub fn reader_from_bytes(data: &[u8]) -> DapReader<Cursor<Vec<u8>>>;

    /// Construct a valid DAP message frame
    pub fn frame_message(msg: &impl Serialize) -> Vec<u8>;
}
```

## Usage Examples

### Basic Client Usage

```rust
use transport2::{connect, Message, OutgoingMessage};

#[tokio::main]
async fn main() -> Result<()> {
    let (mut reader, mut writer) = connect("127.0.0.1:5678").await?;

    // Send initialize request
    writer.send(OutgoingMessage::Request(Request {
        seq: 1,
        command: "initialize".into(),
        arguments: Some(json!({ "clientID": "my-client" })),
    })).await?;

    // Read messages
    while let Some(msg) = reader.next().await {
        match msg? {
            Message::Response(resp) => println!("Got response: {:?}", resp),
            Message::Event(evt) => println!("Got event: {:?}", evt),
            Message::Request(req) => println!("Got reverse request: {:?}", req),
        }
    }

    Ok(())
}
```

### Upstream Integration Pattern

The debugger crate would use transport2 like this:

```rust
use transport2::{split, DapReader, DapWriter, Message};
use tokio::sync::mpsc;

struct DebugSession<R, W> {
    reader: DapReader<R>,
    writer: DapWriter<W>,
    pending_requests: HashMap<Seq, oneshot::Sender<Response>>,
    event_tx: mpsc::Sender<Event>,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> DebugSession<R, W> {
    async fn run(&mut self) {
        while let Some(msg) = self.reader.next().await {
            match msg? {
                Message::Response(resp) => {
                    if let Some(tx) = self.pending_requests.remove(&resp.request_seq) {
                        let _ = tx.send(resp);
                    }
                }
                Message::Event(evt) => {
                    let _ = self.event_tx.send(evt).await;
                }
                Message::Request(_) => {
                    // Handle reverse requests
                }
            }
        }
    }

    async fn send_request(&mut self, body: RequestBody) -> oneshot::Receiver<Response> {
        let seq = self.next_seq();
        let (tx, rx) = oneshot::channel();
        self.pending_requests.insert(seq, tx);
        self.writer.send(/* ... */).await?;
        rx
    }
}
```

## Migration Path

1. Create transport2 crate with core codec functionality
2. Add DapTransport trait and TCP implementation
3. Port in-memory transport for testing
4. Create integration tests using debugpy
5. Update debugger crate to use transport2 (new async version)
6. Deprecate old transport crate

## Dependencies

```toml
[dependencies]
bytes = "1.10"
futures = "0.3"
pin-project-lite = "0.2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
tokio = { version = "1.48", features = ["net", "io-util"] }
tokio-util = { version = "0.7", features = ["codec"] }
tracing = "0.1"

[dev-dependencies]
tokio = { version = "1.48", features = ["full", "test-util"] }
```

## Differences from Current transport Crate

| Aspect | transport (current) | transport2 (new) |
|--------|---------------------|------------------|
| Async model | Blocking I/O + threads | tokio async/await |
| Message parsing | Custom HandWrittenReader | tokio-util Decoder |
| Request tracking | Built-in RequestStore | Delegated to upstream |
| Event routing | Built-in channels | Delegated to upstream |
| Split pattern | Not first-class | Primary API |
| Testing | Custom InMemoryTransport | tokio DuplexStream |
| Buffer management | Vec<u8> | bytes::BytesMut |

## Open Questions

1. **Type reuse**: Should we re-use types from transport crate or the external `dap` crate?
   - Recommendation: Define our own compatible types for control, re-export from lib.rs

2. **Backpressure**: Should DapWriter buffer messages or apply backpressure?
   - Recommendation: Use tokio-util's built-in buffering with configurable limits

3. **Graceful shutdown**: How to signal reader/writer to stop?
   - Recommendation: Dropping the writer closes the connection; reader returns None
