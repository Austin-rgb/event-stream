use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::to_string;
use std::pin::Pin;
use std::{future::Future, marker::PhantomData};
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

pub trait Subscribable: DeserializeOwned + Send + Sync + 'static {
    const SUBJECT: &'static str;
}

#[async_trait]
pub trait Subscriber<T: Subscribable>: Send + Sync + Sized + 'static {
    // Now receives the full Event<T> with metadata
    async fn on_message(&self, metadata: &Event<T>, subject: &str);

    async fn subscribe(self, es: Arc<dyn EventStream>) -> Result<(), EventError> {
        struct MessageHandler<C: Subscriber<T> + Send + Sync + 'static, T: Subscribable> {
            subscriber: C,
            _marker: PhantomData<T>,
        }

        #[async_trait]
        impl<C: Subscriber<T> + Send + Sync + 'static, T: Subscribable> Handler for MessageHandler<C, T> {
            async fn handle(&self, subject: String, message: Vec<u8>) {
                // Deserialize the full Event<T>
                match serde_json::from_slice::<Event<T>>(&message) {
                    Ok(event) => {
                        // Pass both payload and metadata
                        self.subscriber.on_message(&event, &subject).await;
                    }
                    Err(e) => {
                        eprintln!("Failed to deserialize event on {}: {}", subject, e);
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
