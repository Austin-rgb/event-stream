use std::sync::Arc;

use async_nats::{Client, jetstream};
pub use async_nats::Error;

pub struct NatsEventStream {
    client: Client,
    group: String,
}

impl NatsEventStream {
    pub async fn new(url: &str) -> Result<Self, Error> {
        let client = async_nats::connect(url).await?;
        Ok(Self {
            client,
            group: uuid::Uuid::new_v4().to_string(),
        })
    }
    pub async fn with_group(self, group: String) -> Self {
        Self { group, ..self }
    }
}

use futures::StreamExt;

use crate::{BoxFuture, EventError, EventStream, Handler};

impl EventStream for NatsEventStream {
    fn publish<'a>(
        &'a self,
        subject: String,
        payload: Vec<u8>,
    ) -> BoxFuture<'a, Result<(), EventError>> {
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
        handler: Arc<dyn Handler>,
    ) -> BoxFuture<'a, Result<(), EventError>> {
        Box::pin(async move {
            let mut sub = self
                .client
                .queue_subscribe(subject, self.group.clone())
                .await?;

            tokio::spawn(async move {
                while let Some(msg) = sub.next().await {
                    handler
                        .handle(msg.subject.into_string(), msg.payload.to_vec())
                        .await;
                }
            });

            Ok(())
        })
    }
}


pub struct NatsAloStream {
    js: jetstream::Context,
    group: String,
    stream_name: String,
}

impl NatsAloStream {
    pub async fn new(url: &str) -> Result<Self, Error> {
        let client = async_nats::connect(url).await?;
        let js = jetstream::new(client.clone());
        
        // ensure default stream exists - change subjects to your needs
        // or create it externally with `nats stream add`
        let _ = js.get_or_create_stream(jetstream::stream::Config {
            name: "EVENTS".to_string(),
            subjects: vec!["events.>".to_string(), "payment.>".to_string()],
            ..Default::default()
        }).await;

        Ok(Self {
            js,
            group: uuid::Uuid::new_v4().to_string(),
            stream_name: "EVENTS".to_string(),
        })
    }

    pub fn with_group(self, group: String) -> Self {
        Self { group, ..self }
    }
    
    pub fn with_stream(mut self, stream: String) -> Self {
        self.stream_name = stream;
        self
    }
}

impl EventStream for NatsAloStream {
    fn publish<'a>(
        &'a self,
        subject: String,
        payload: Vec<u8>,
    ) -> BoxFuture<'a, Result<(), EventError>> {
        Box::pin(async move {
            self.js
                .publish(subject, payload.into())
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            Ok(())
        })
    }

    fn subscribe<'a>(
        &'a self,
        subject: String,
        handler: Arc<dyn Handler>,
    ) -> BoxFuture<'a, Result<(), EventError>> {
        Box::pin(async move {
            let stream = self.js.get_stream(self.stream_name.clone()).await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            // durable_name = your consumer group. Same group = load balanced
            let consumer = stream.get_or_create_consumer(
                &self.group,
                jetstream::consumer::pull::Config {
                    durable_name: Some(self.group.clone()),
                    // filter only this subject if needed
                    filter_subject: subject.clone(),
                    deliver_policy: jetstream::consumer::DeliverPolicy::All,
                    ack_policy: jetstream::consumer::AckPolicy::Explicit,
                    max_ack_pending: 1000,
                    ..Default::default()
                },
            ).await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            let mut messages = consumer.messages().await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            // spawn persistent loop
            tokio::spawn(async move {
                while let Some(Ok(msg)) = messages.next().await {
                    handler.handle(msg.subject.to_string(), msg.payload.to_vec()).await;
                    // only ack after handler succeeded
                    let _ = msg.ack().await;
                }
            });

            Ok(())
        })
    }
}