use async_trait::async_trait;
use bytes::Bytes;
use dashmap::DashMap;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

pub type BoxFuture<'a, T> = Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

type Msg = Arc<(String, Bytes)>; // Arc so clones are cheap
type Tx = mpsc::Sender<Msg>;

pub struct LocalEventStream {
    // subject -> list of senders. DashMap shard locks only on insert/remove.
    subs: DashMap<String, Vec<Tx>>,
    _tasks: tokio::sync::Mutex<JoinSet<()>>,
    channel_capacity: usize,
}

impl LocalEventStream {
    pub fn new(channel_capacity: usize) -> Self {
        Self {
            subs: DashMap::new(),
            _tasks: tokio::sync::Mutex::new(JoinSet::new()),
            channel_capacity,
        }
    }

    // 8192 = ~8MB per subscriber if 1KB avg msg. Tune based on RAM.
    pub fn reliable() -> Self {
        Self::new(8192)
    }
}

impl EventStream for LocalEventStream {
    fn publish<'a>(
        &'a self,
        subject: String,
        payload: Vec<u8>,
    ) -> BoxFuture<'a, Result<(), EventError>> {
        Box::pin(async move {
            let msg = Arc::new((subject.clone(), Bytes::from(payload)));

            // Fast path: no global lock. Only shard lock for this subject.
            let Some(senders) = self.subs.get(&subject) else {
                return Ok(()); // No subscribers = drop. Not an error.
            };

            // Fan-out. If any queue is full, await = backpressure.
            // This is the reliability guarantee: publish won't return until all
            // subscribers have space in their queue.
            for tx in senders.iter() {
                if tx.send(msg.clone()).await.is_err() {
                    // Receiver dropped. Remove it lazily on next publish or subscribe.
                    // For now just ignore to avoid write lock during publish.
                }
            }
            Ok(())
        })
    }

    fn subscribe<'a>(
        &'a self,
        subject: String,
        handler: Arc<dyn Handler>,
    ) -> BoxFuture<'a, Result<(), EventError>> {
        Box::pin(async move {
            let (tx, mut rx) = mpsc::channel::<Msg>(self.channel_capacity);

            // Insert sender. Need write lock on shard, but only briefly.
            self.subs
               .entry(subject.clone())
               .or_insert_with(Vec::new)
               .push(tx);

            let mut tasks = self._tasks.lock().await;
            tasks.spawn(async move {
                // Ordered, lossless consumption. If handler is slow, rx will fill
                // and publishers will await on send(). That's backpressure.
                while let Some(msg) = rx.recv().await {
                    let (subj, payload) = &*msg; // Arc deref
                    handler.handle(subj.clone(), payload.to_vec()).await;
                }
            });

            Ok(())
        })
    }
}

impl Drop for LocalEventStream {
    fn drop(&mut self) {
        // Abort all handler tasks when bus drops
        if let Ok(mut tasks) = self._tasks.try_lock() {
            tasks.abort_all();
        }
    }
}
