use std::future::Future;
use std::pin::Pin;

pub type EventHandler =
    Box<dyn Fn(Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait EventStream: Send + Sync {
    type Error: Send + Sync + 'static;

    fn publish<'a>(
        &'a self,
        subject: String,
        payload: Vec<u8>,
    ) -> BoxFuture<'a, Result<(), Self::Error>>;

    fn subscribe<'a>(
        &'a self,
        subject: String,
        handler: EventHandler,
    ) -> BoxFuture<'a, Result<(), Self::Error>>;
}

pub mod nats;
