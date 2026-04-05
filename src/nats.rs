use async_nats::Client;
pub use async_nats::Error;
use bytes::Bytes;
pub struct NatsEventStream {
    client: Client,
}

impl NatsEventStream {
    pub async fn new(url: &str) -> Result<Self, async_nats::Error> {
        let client = async_nats::connect(url).await?;
        Ok(Self { client })
    }
}

use futures::StreamExt;

use crate::{BoxFuture, EventHandler, EventStream};

impl EventStream for NatsEventStream {
    type Error = async_nats::Error;

    fn publish<'a>(
        &'a self,
        subject: String,
        payload: Vec<u8>,
    ) -> BoxFuture<'a, Result<(), Self::Error>> {
        Box::pin(async move {
            self.client
                .publish(subject, payload.into())
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        })
    }

    fn subscribe<'a>(
        &'a self,
        subject: String,
        handler: EventHandler,
    ) -> BoxFuture<'a, Result<(), Self::Error>> {
        Box::pin(async move {
            let mut sub = self.client.subscribe(subject).await?;

            tokio::spawn(async move {
                while let Some(msg) = sub.next().await {
                    (handler)(msg.payload.to_vec()).await;
                }
            });

            Ok(())
        })
    }
}
