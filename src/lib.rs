use async_trait::async_trait;
use serde::Serialize;
use serde_json::to_string;
use std::future::Future;
use std::pin::Pin;
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
