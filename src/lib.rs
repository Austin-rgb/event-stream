use async_trait::async_trait;
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
