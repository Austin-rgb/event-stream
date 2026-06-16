use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::to_string;
use std::future::Future;
use std::pin::Pin;
mod envelop;
pub use envelop::{Event, EventMetaData};
use std::sync::Arc;
pub type EventError = Box<dyn std::error::Error + Send + Sync>;
pub type EventHandler =
    Box<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

#[async_trait]
pub trait Handler: Send + Sync + 'static {
    async fn handle(&self, subject: String, message: Vec<u8>);
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait EventStream: Send + Sync {
    fn publish<'a>(
        &'a self,
        subject: String,
        payload: Vec<u8>,
    ) -> BoxFuture<'a, Result<(), EventError>>;

    fn subscribe<'a>(
        &'a self,
        subject: String,
        handler: Arc<dyn Handler>,
    ) -> BoxFuture<'a, Result<(), EventError>>;
}

pub mod nats;
pub use nats::NatsEventStream;

#[async_trait]
pub trait Publishable: Serialize {
    const SUBJECT: &'static str;

    async fn publish(&self, bus: Arc<dyn EventStream>) -> Result<(), EventError> {
        bus.publish(
            Self::SUBJECT.to_string(),
            to_string(self).unwrap().into_bytes(),
        )
        .await
    }

    async fn subscribe(
        bus: Arc<dyn EventStream>,
        handler: Arc<dyn Handler>,
    ) -> Result<(), EventError> {
        bus.subscribe(Self::SUBJECT.to_string(), handler).await
    }
}

#[async_trait]
pub trait Subscribable: DeserializeOwned + Send + Sync + 'static {
    const SUBJECT: &'static str;

    // Now receives the full Event<T> with metadata
    async fn on_message(&self, metadata: &EventMetaData, subject: &str);

    async fn subscribe(es: Arc<dyn EventStream>) -> Result<(), EventError> {
        struct MessageHandler<T: Subscribable> {
            _marker: std::marker::PhantomData<T>,
        }

        #[async_trait]
        impl<T: Subscribable> Handler for MessageHandler<T> {
            async fn handle(&self, subject: String, message: Vec<u8>) {
                // Deserialize the full Event<T>
                match serde_json::from_slice::<Event<T>>(&message) {
                    Ok(event) => {
                        // Pass both payload and metadata
                        event.payload.on_message(&event.metadata, &subject).await;
                    }
                    Err(e) => {
                        eprintln!("Failed to deserialize event on {}: {}", subject, e);
                    }
                }
            }
        }

        let handler = Arc::new(MessageHandler::<Self> {
            _marker: std::marker::PhantomData,
        });

        es.subscribe(Self::SUBJECT.to_string(), handler).await
    }
}
