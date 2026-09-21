use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use std::pin::Pin;
use std::{future::Future, marker::PhantomData};
mod envelop;
pub use envelop::{Event, EventMetaData, Identifier};
use std::sync::Arc;
pub type EventError = Box<dyn std::error::Error + Send + Sync>;
pub type EventHandler =
    Box<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

#[async_trait]
pub trait Handler: Send + Sync + 'static {
    async fn handle(&self, subject: String, message: Vec<u8>) -> Result<(), EventError>;
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
mod local;
pub use local::LocalEventStream;
mod nats;
pub use nats::{NatsAloStream, NatsEventStream};

pub trait EventType: Serialize + DeserializeOwned + Send + Sync + 'static {
    const SUBJECT: &'static str;
}

#[async_trait]
pub trait Subscriber<T: EventType>: Send + Sync + Sized + 'static {
    // Now receives the full Event<T> with metadata
    async fn on_message(&self, event: Event<T>, subject: &str) -> Result<(), EventError>;

    async fn subscribe(self, es: Arc<dyn EventStream>) -> Result<(), EventError> {
        struct MessageHandler<C: Subscriber<T> + Send + Sync + 'static, T: EventType> {
            subscriber: C,
            _marker: PhantomData<T>,
        }

        #[async_trait]
        impl<C: Subscriber<T> + Send + Sync + 'static, T: EventType> Handler for MessageHandler<C, T> {
            async fn handle(&self, subject: String, message: Vec<u8>) -> Result<(), EventError> {
                // Deserialize the full Event<T>
                match serde_json::from_slice::<Event<T>>(&message) {
                    Ok(event) => {
                        // Pass both payload and metadata
                        self.subscriber.on_message(event, &subject).await
                    }
                    Err(e) => {
                        tracing::error!("Failed to deserialize event on {}: {}", subject, e);
                        Err(Box::new(e))
                    }
                }
            }
        }

        let handler = Arc::new(MessageHandler::<Self, T> {
            subscriber: self,
            _marker: PhantomData,
        });

        es.subscribe(T::SUBJECT.to_string(), handler).await
    }
}
