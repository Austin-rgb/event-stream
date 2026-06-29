use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;
use uuid::Uuid;

use crate::{EventStream, Publishable};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetaData {
    pub event_id: Uuid,
    pub event_version: String,
    pub occurred_at: Timestamp,
    pub producer: Option<String>,
    pub correlation_id: Option<Uuid>,
    pub trace_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub audience: Vec<Uuid>,
    pub session_id: Option<Uuid>,
}

impl EventMetaData {
    pub fn new() -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_version: "v1".to_string(),
            occurred_at: Timestamp::now(),
            producer: None,
            correlation_id: None,
            trace_id: None,
            user_id: None,
            session_id: None,
            audience: Vec::new(),
        }
    }
    
    pub fn with_producer(mut self, producer: impl Into<String>)->Self{
        self.producer = Some(producer.into());
        self
    }

    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    pub fn with_trace_id(mut self, id: Uuid) -> Self {
        self.trace_id = Some(id);
        self
    }

    pub fn with_user_id(mut self, id: Uuid) -> Self {
        self.user_id = Some(id);
        self
    }

    pub fn with_session_id(mut self, id: Uuid) -> Self {
        self.session_id = Some(id);
        self
    }
    pub fn with_audience(mut self, aud: Vec<Uuid>) -> Self {
        self.audience = aud;
        self
    }
}

#[derive(Serialize, Deserialize)]
pub struct Event<T> {
    pub metadata: EventMetaData,
    pub payload: T,
}

impl<T: Publishable + Sync> Event<T> {
    pub fn new(payload: T) -> Self {
        let metadata = EventMetaData::new();
        Self { metadata, payload }
    }
    pub async fn publish(
        &self,
        es: Arc<dyn EventStream>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match es
            .publish(
                T::SUBJECT.to_string(),
                serde_json::to_string(self).unwrap().into_bytes(),
            )
            .await
        {
            Ok(_) => (),
            Err(e) => eprintln!("Error in publishing event: {e}"),
        };
        Ok(())
    }
    
    pub fn with_producer(mut self, producer: impl Into<String>)->Self{
        self.metadata = self.metadata.with_producer(producer);
        self
    }

    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.metadata = self.metadata.with_correlation_id(id);
        self
    }

    pub fn with_trace_id(mut self, id: Uuid) -> Self {
        self.metadata = self.metadata.with_trace_id(id);
        self
    }

    pub fn with_user_id(mut self, id: Uuid) -> Self {
        self.metadata = self.metadata.with_user_id(id);
        self
    }

    pub fn with_session_id(mut self, id: Uuid) -> Self {
        self.metadata = self.metadata.with_session_id(id);
        self
    }
    pub fn with_audience(mut self, aud: Vec<Uuid>) -> Self {
        self.metadata = self.metadata.with_audience(aud);
        self
    }
}
