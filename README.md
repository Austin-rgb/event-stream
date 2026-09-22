# typed-eventbus

Transport-agnostic event streaming with **typed events**, rich metadata envelopes, in-process pub/sub, and first-class NATS (core + JetStream) support.

[![Crates.io](https://img.shields.io/crates/v/typed-eventbus.svg)](https://crates.io/crates/typed-eventbus)
[![Documentation](https://docs.rs/typed-eventbus/badge.svg)](https://docs.rs/typed-eventbus)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

## Features

- **Typed events** – each event type declares its subject via the `EventType` trait
- **Metadata envelope** – every message carries event ID, version, timestamp, producer, correlation/trace/user/session IDs, and an audience list
- **Transport-agnostic** – the same application code works against an in-memory bus or NATS
- **LocalEventStream** – high-performance in-process bus with back-pressure
- **NatsEventStream** – core NATS pub/sub with queue groups (load balancing)
- **NatsAloStream** – JetStream “at-least-once” delivery with durable consumers and explicit acks
- Built on modern async Rust (`tokio`, `async-nats`, `jiff`, `serde`)

## Installation

```toml
[dependencies]
typed-eventbus = "0.3.1"
```

> **Note:** The crate currently depends on NATS client libraries. A pure-local build (without NATS) is not yet feature-gated.

Requires a recent Rust toolchain (edition 2024).

## Quick start

### 1. Define an event type

```rust
use serde::{Deserialize, Serialize};
use typed_eventbus::EventType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCreated {
    pub user_id: String,
    pub email: String,
}

impl EventType for UserCreated {
    const SUBJECT: &'static str = "users.created";
}
```

### 2. Publish an event

```rust
use std::sync::Arc;
use typed_eventbus::{Event, EventStream, LocalEventStream};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bus: Arc<dyn EventStream> = Arc::new(LocalEventStream::reliable());

    let event = Event::new(UserCreated {
        user_id: "u_123".into(),
        email: "alice@example.com".into(),
    })
    .with_producer("user-service")
    .with_correlation_id(uuid::Uuid::new_v4());

    event.publish(bus).await?;
    Ok(())
}
```

### 3. Subscribe with a typed handler

```rust
use std::sync::Arc;
use async_trait::async_trait;
use typed_eventbus::{
    Event, EventError, EventStream, EventType, LocalEventStream, Subscriber,
};

struct UserCreatedHandler;

#[async_trait]
impl Subscriber<UserCreated> for UserCreatedHandler {
    async fn on_message(
        &self,
        event: Event<UserCreated>,
        subject: &str,
    ) -> Result<(), EventError> {
        println!(
            "[{}] user {} created (event_id={})",
            subject,
            event.payload.user_id,
            event.metadata.event_id
        );
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bus: Arc<dyn EventStream> = Arc::new(LocalEventStream::reliable());

    // Subscribe – the handler is moved into the subscription
    UserCreatedHandler.subscribe(bus.clone()).await?;

    // Publish as before…
    let event = Event::new(UserCreated {
        user_id: "u_123".into(),
        email: "alice@example.com".into(),
    });
    event.publish(bus).await?;

    // Give the handler a moment to run
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    Ok(())
}
```

## Transports

### Local (in-process)

```rust
use typed_eventbus::LocalEventStream;

// Bounded channels (back-pressure). 8192 is a reasonable default.
let bus = LocalEventStream::reliable();

// Or choose your own capacity
let bus = LocalEventStream::new(4096);
```

- Exact subject matching only (no wildcards).
- Publish blocks when any subscriber’s channel is full → natural back-pressure.
- Dead subscribers are cleaned up automatically.

### NATS core (`NatsEventStream`)

Fire-and-forget messaging with optional queue groups for load balancing.

```rust
use typed_eventbus::NatsEventStream;

let bus = NatsEventStream::new("nats://127.0.0.1:4222")
    .await?
    .with_group("my-service".into());   // same group name → load-balanced
```

- Handler errors are logged and **not** retried (core NATS has no redelivery).
- Each process gets a random queue group by default; call `.with_group(...)` to share work across instances.

### NATS JetStream – at-least-once (`NatsAloStream`)

Durable consumers, explicit acks, and automatic redelivery on handler failure.

```rust
use typed_eventbus::NatsAloStream;

let bus = NatsAloStream::new("nats://127.0.0.1:4222", "EVENTS".into())
    .await?
    .with_group("order-workers".into());
```

**Important subject convention**

On construction the stream is created with the subject filter:

```text
{stream_name_lowercased}.>
```

Therefore every subject you publish **must** be prefixed with the lower-cased stream name, for example:

| Stream name | Event subject you should use      |
|-------------|-----------------------------------|
| `EVENTS`    | `events.users.created`            |
| `ORDERS`    | `orders.payment.completed`        |

If the prefix is missing the message will not be stored in the stream.

Durable consumer names are derived as `{group}__{sanitized_subject}` so that:

- Multiple subjects under the same group stay independent.
- Multiple instances with the same group share a durable consumer (load balancing).
- Collisions after sanitization are detected and rejected.

## Core types

| Type / Trait       | Purpose                                                                 |
|--------------------|-------------------------------------------------------------------------|
| `EventType`        | Marker trait – implement for every payload and declare `const SUBJECT` |
| `Event<T>`         | Envelope that holds `EventMetaData` + payload                          |
| `EventMetaData`    | event_id, version, occurred_at, producer, correlation/trace/user/session IDs, audience |
| `EventStream`      | Low-level transport trait (`publish` / `subscribe` with raw bytes)     |
| `Handler`          | Low-level callback receiving `(subject, bytes)`                        |
| `Subscriber<T>`    | High-level typed handler – implement `on_message`                      |
| `LocalEventStream` | In-memory implementation                                               |
| `NatsEventStream`  | Core NATS implementation                                               |
| `NatsAloStream`    | JetStream at-least-once implementation                                 |

### Working with metadata

```rust
use uuid::Uuid;
use typed_eventbus::{Event, Identifier};

let event = Event::new(my_payload)
    .with_producer("checkout-service")
    .with_correlation_id(Uuid::new_v4())
    .with_trace_id(Uuid::new_v4())
    .with_user_id(user_uuid)
    .with_session_id(session_uuid)
    .with_audience(vec!["admin", "audit-log"])   // &str or Uuid
    .add_audience(Uuid::new_v4());               // append one more
```

`Identifier` accepts both UUIDs and free-form tags.

## Error handling semantics

| Scenario                        | Local              | NATS core          | JetStream (ALO)          |
|---------------------------------|--------------------|--------------------|--------------------------|
| Deserialization failure         | Logged, dropped    | Logged, dropped    | Logged, dropped          |
| Handler returns `Err`           | Logged, ignored    | Logged, no retry   | **Not acked → redelivered** |
| No subscribers                  | Publish succeeds   | Publish succeeds   | Publish succeeds         |

Deserialization errors are intentionally absorbed: redelivery cannot fix a permanently unreadable payload.

## Design notes & limitations

- **No unsubscribe API** – subscriptions live for the lifetime of the process (or until the bus is dropped).
- **Local subjects are exact-match only** – wildcards are not supported in the in-memory bus.
- **JSON only** – payloads are serialized with `serde_json`. Binary codecs are not yet supported.
- **JetStream subject prefix** – see the table above; this is the most common source of “messages disappear”.
- **Edition 2024** – the crate currently requires a toolchain that supports the 2024 edition.

## License

MIT

## Links

- Repository: https://github.com/Austin-rgb/event-stream
- Documentation: https://docs.rs/typed-eventbus

