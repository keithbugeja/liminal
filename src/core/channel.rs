use crate::config::types::ChannelType;
use async_trait::async_trait;
use flume;
use std::sync::Mutex;
use tokio::sync::{broadcast, mpsc};

#[derive(Debug)]
pub enum PublishError<M> {
    BroadcastError(broadcast::error::SendError<M>),
    MpscError(mpsc::error::SendError<M>),
    FlumeError(flume::SendError<M>),
    FanoutError(mpsc::error::SendError<M>),
}

pub enum Subscriber<M> {
    Broadcast(broadcast::Receiver<M>),
    Mpsc(mpsc::Receiver<M>),
    Flume(flume::Receiver<M>),
    Fanout(mpsc::Receiver<M>),
}

impl<M> Subscriber<M>
where
    M: Clone,
{
    /// Receive the next message from the channel.
    /// - mpsc: returns `None` if the channel is closed.
    /// - broadcast: skips lagged, returns `None` if the channel is closed.
    /// - flume: returns `None` if disconnected.
    /// - fanout: returns `None` if the channel is closed.
    pub async fn recv(&mut self) -> Option<M> {
        match self {
            Subscriber::Mpsc(rx) => rx.recv().await,
            Subscriber::Broadcast(rx) => match rx.recv().await {
                Ok(msg) => Some(msg),
                Err(broadcast::error::RecvError::Lagged(_)) => None,
                Err(broadcast::error::RecvError::Closed) => None,
            },
            Subscriber::Flume(rx) => match rx.recv_async().await {
                Ok(msg) => Some(msg),
                Err(flume::RecvError::Disconnected) => None,
            },
            Subscriber::Fanout(rx) => rx.recv().await,
        }
    }

    pub async fn try_recv(&mut self) -> Option<M> {
        match self {
            Subscriber::Mpsc(rx) => match rx.try_recv() {
                Ok(msg) => Some(msg),
                _ => None,
            }
            Subscriber::Broadcast(rx) => match rx.try_recv() {
                Ok(msg) => Some(msg),
                _ => None,
            },
            Subscriber::Flume(rx) => match rx.try_recv() {
                Ok(msg) => Some(msg),
                _ => None,
            }
            Subscriber::Fanout(rx) => match rx.try_recv() {
                Ok(msg) => Some(msg),
                _ => None,
            }
        }
    }
}

#[async_trait]
pub trait PubSubChannel<M>: Send + Sync {
    /// Publish a message to the channel.
    async fn publish(&self, msg: M) -> Result<(), PublishError<M>>;

    /// Subscribe to the channel to get a fresh receiver.
    fn subscribe(&self) -> Subscriber<M>;
}

/// MPSC / point-to-point channel
pub struct MpscChannel<M> {
    sender: mpsc::Sender<M>,
    receiver: Mutex<Option<mpsc::Receiver<M>>>,
}

impl<M> MpscChannel<M> {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self {
            sender,
            receiver: Mutex::new(Some(receiver)),
        }
    }
}

#[async_trait]
impl<M> PubSubChannel<M> for MpscChannel<M>
where
    M: Send + 'static,
{
    async fn publish(&self, msg: M) -> Result<(), PublishError<M>> {
        self.sender.send(msg).await.map_err(PublishError::MpscError)
    }

    fn subscribe(&self) -> Subscriber<M> {
        let mut guard = self
            .receiver
            .lock()
            .expect("mpsc: lock failed, poisoned receiver mutex!");

        Subscriber::Mpsc(
            guard
                .take()
                .expect("mpsc: subscribe() called more than once"),
        )
    }
}

/// Broacast channel / fan-out channel (at-most-once)
pub struct BroadcastChannel<M> {
    sender: broadcast::Sender<M>,
}

impl<M> BroadcastChannel<M>
where
    M: Clone + Send + Sync + 'static,
{
    pub fn new(capacity: usize) -> Self {
        let (sender, _receiver) = broadcast::channel(capacity);
        Self { sender }
    }
}

#[async_trait]
impl<M> PubSubChannel<M> for BroadcastChannel<M>
where
    M: Clone + Send + Sync + 'static,
{
    async fn publish(&self, msg: M) -> Result<(), PublishError<M>> {
        self.sender
            .send(msg)
            .map(|_| ())
            .map_err(PublishError::BroadcastError)
    }

    fn subscribe(&self) -> Subscriber<M> {
        Subscriber::Broadcast(self.sender.subscribe())
    }
}

/// Flume channel / reliable fan-out channel (at-least-once)
pub struct FlumeChannel<M> {
    sender: flume::Sender<M>,
    receiver: flume::Receiver<M>,
}

impl<M> FlumeChannel<M> {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = flume::bounded(capacity);
        Self { sender, receiver }
    }
}

#[async_trait]
impl<M> PubSubChannel<M> for FlumeChannel<M>
where
    M: Send + 'static,
{
    async fn publish(&self, msg: M) -> Result<(), PublishError<M>> {
        self.sender
            .send_async(msg)
            .await
            .map_err(PublishError::FlumeError)
    }

    fn subscribe(&self) -> Subscriber<M> {
        Subscriber::Flume(self.receiver.clone())
    }
}

/// Fanout channel / reliable fan-out channel (at-least-once)
pub struct FanoutChannel<M> {
    capacity: usize,
    senders: tokio::sync::Mutex<Vec<mpsc::Sender<M>>>,
}

impl<M> FanoutChannel<M> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            senders: tokio::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl<M> PubSubChannel<M> for FanoutChannel<M>
where
    M: Clone + Send + 'static,
{
    async fn publish(&self, msg: M) -> Result<(), PublishError<M>> {
        let senders = {
            let guard = self.senders.lock().await;
            guard.clone()
        };

        for sender in senders.iter() {
            sender
                .send(msg.clone())
                .await
                .map_err(PublishError::FanoutError)?;
        }

        Ok(())
    }

    fn subscribe(&self) -> Subscriber<M> {
        let (sender, receiver) = mpsc::channel(self.capacity);

        let mut guard = futures::executor::block_on(self.senders.lock());
        guard.push(sender);
        Subscriber::Fanout(receiver)
    }
}

// Enum wrapper for different channel types
pub enum Channel<M> {
    Broadcast(BroadcastChannel<M>),
    Mpsc(MpscChannel<M>),
    Flume(FlumeChannel<M>),
    Fanout(FanoutChannel<M>),
}

impl<M> Channel<M>
where
    M: Clone + Send + Sync + 'static,
{
    pub fn new(kind: ChannelType, capacity: usize) -> Self {
        match kind {
            ChannelType::Broadcast => Channel::Broadcast(BroadcastChannel::new(capacity)),
            ChannelType::Direct => Channel::Mpsc(MpscChannel::new(capacity)),
            ChannelType::Shared => Channel::Flume(FlumeChannel::new(capacity)),
            ChannelType::Fanout => Channel::Fanout(FanoutChannel::new(capacity)),
        }
    }
}

#[async_trait]
impl<M> PubSubChannel<M> for Channel<M>
where
    M: Clone + Send + Sync + 'static,
{
    async fn publish(&self, msg: M) -> Result<(), PublishError<M>> {
        match self {
            Channel::Broadcast(bc) => bc.publish(msg).await,
            Channel::Mpsc(mc) => mc.publish(msg).await,
            Channel::Flume(fc) => fc.publish(msg).await,
            Channel::Fanout(fc) => fc.publish(msg).await,
        }
    }

    fn subscribe(&self) -> Subscriber<M> {
        match self {
            Channel::Broadcast(bc) => bc.subscribe(),
            Channel::Mpsc(mc) => mc.subscribe(),
            Channel::Flume(fc) => fc.subscribe(),
            Channel::Fanout(fc) => fc.subscribe(),
        }
    }
}
