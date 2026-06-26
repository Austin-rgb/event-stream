# event-stream

Transport-agnostic event streaming for Rust applications.

"event-stream" provides a lightweight abstraction for event-driven architectures with support for:

- Strongly typed events
- Event metadata envelopes
- In-memory pub/sub
- NATS-backed messaging
- Typed subscribers
- Transport-independent domain code

## Features

### EventStream abstraction

Publishers and subscribers depend on a common trait:
```rust
pub trait EventStream {
    fn publish(...);
    fn subscribe(...);
}
```

This allows applications to switch between local and distributed transports without changing domain logic.

---

### Event metadata

Every event can carry metadata such as:

- Event ID
- Event version
- Timestamp
- Producer
- Correlation ID
- Trace ID
- User ID
- Session ID
- Audience

Example:
```rust
let metadata = EventMetaData::new("orders-service")
    .with_user_id(user_id)
    .with_correlation_id(correlation_id);
```
---

### Publishing events

Define an event payload:
```rust
#[derive(Serialize)]
struct OrderCreated {
    order_id: String,
}

#[async_trait]
impl Publishable for OrderCreated {
    const SUBJECT: &'static str = "orders.created";
}
```
Publish:
```rust
let event = Event::new(
    EventMetaData::new("orders-service"),
    OrderCreated {
        order_id: "123".into(),
    },
);

event.publish(bus.clone()).await?;
```
---

### Subscribing to events

Define a payload:
```rust
#[derive(Deserialize)]
struct OrderCreated {
    order_id: String,
}

impl Subscribable for OrderCreated {
    const SUBJECT: &'static str = "orders.created";
}
```
Create a subscriber:
```rust
struct AuditSubscriber;

#[async_trait]
impl Subscriber<OrderCreated> for AuditSubscriber {
    async fn on_message(
        &self,
        event: Event<OrderCreated>,
        subject: &str,
    ) {
        println!(
            "received {} on {}",
            event.payload.order_id,
            subject
        );
    }
}
```
Register:

AuditSubscriber.subscribe(bus.clone()).await?;

---

### Local event stream

Useful for testing and single-process deployments.
```rust
let bus = Arc::new(LocalEventStream::reliable());
```
The local implementation provides backpressure by awaiting when subscriber queues are full.

---

### NATS event stream

Connect to a NATS server:
```rust
let bus = Arc::new(
    NatsEventStream::new("nats://localhost:4222").await?
);
```
Publishers and subscribers remain unchanged.

---

## Design goals

- Transport-independent domain events
- Strong typing
- Simple pub/sub API
- Rich event metadata
- Local-first development
- NATS for distributed deployments

License

MIT
