//! Bounded local event subscriptions with explicit backpressure.

use metis_core::error::{ErrorCode, MetisError, Result};
use std::collections::BTreeMap;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::time::Duration;

/// Maximum number of subscriptions in one hub.
pub const MAX_SUBSCRIPTIONS: usize = 64;

/// Identifies one event subscription within a hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    /// Returns the stable numeric identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Receiver for one bounded event subscription.
#[derive(Debug)]
pub struct Subscription<E> {
    id: SubscriptionId,
    receiver: Receiver<E>,
}

impl<E> Subscription<E> {
    /// Returns this subscription's identifier.
    #[must_use]
    pub const fn id(&self) -> SubscriptionId {
        self.id
    }

    /// Polls for an event without waiting.
    ///
    /// `Ok(None)` means that no event is currently queued. A disconnected hub
    /// is returned as a typed connection error.
    ///
    /// # Errors
    /// Returns [`ErrorCode::ConnectionClosed`] when the hub was dropped.
    pub fn try_recv(&self) -> Result<Option<E>> {
        match self.receiver.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(MetisError::transport(
                ErrorCode::ConnectionClosed,
                "Event hub closed the subscription",
            )),
        }
    }

    /// Receives one event within a finite deadline.
    ///
    /// # Errors
    /// Returns [`ErrorCode::Timeout`] when no event arrives before `timeout`,
    /// or [`ErrorCode::ConnectionClosed`] when the hub was dropped.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<E> {
        if timeout.is_zero() {
            return Err(MetisError::transport(
                ErrorCode::Timeout,
                "Event subscription receive deadline must be non-zero",
            ));
        }
        self.receiver
            .recv_timeout(timeout)
            .map_err(|error| match error {
                std::sync::mpsc::RecvTimeoutError::Timeout => MetisError::transport(
                    ErrorCode::Timeout,
                    "Event subscription receive deadline elapsed",
                ),
                std::sync::mpsc::RecvTimeoutError::Disconnected => MetisError::transport(
                    ErrorCode::ConnectionClosed,
                    "Event hub closed the subscription",
                ),
            })
    }
}

/// Bounded fan-out event hub for one local host/session.
#[derive(Debug)]
pub struct EventHub<E, const CAPACITY: usize = 16> {
    subscriptions: BTreeMap<SubscriptionId, SyncSender<E>>,
    next_id: Option<u64>,
}

impl<E, const CAPACITY: usize> EventHub<E, CAPACITY> {
    /// Creates an event hub with `CAPACITY` queued events per subscriber.
    ///
    /// # Errors
    /// Returns [`ErrorCode::QueueFull`] when the requested capacity is zero.
    pub fn new() -> Result<Self> {
        if CAPACITY == 0 {
            return Err(MetisError::transport(
                ErrorCode::QueueFull,
                "Event subscription capacity must be non-zero",
            ));
        }
        Ok(Self {
            subscriptions: BTreeMap::new(),
            next_id: Some(1),
        })
    }

    /// Adds a subscriber with its own bounded queue.
    ///
    /// # Errors
    /// Returns a protocol error when the subscription identifier space is
    /// exhausted.
    pub fn subscribe(&mut self) -> Result<Subscription<E>> {
        if self.subscriptions.len() >= MAX_SUBSCRIPTIONS {
            return Err(MetisError::transport(
                ErrorCode::QueueFull,
                "Event hub subscription limit is full",
            ));
        }
        let id = self.next_id.map(SubscriptionId).ok_or_else(|| {
            MetisError::protocol(
                ErrorCode::SequenceMismatch,
                "Event subscription identifier space is exhausted",
            )
        })?;
        self.next_id = id.0.checked_add(1);
        let (sender, receiver) = sync_channel(CAPACITY);
        let previous = self.subscriptions.insert(id, sender);
        debug_assert!(previous.is_none(), "subscription identifiers are unique");
        Ok(Subscription { id, receiver })
    }

    /// Removes a subscriber before another event can be delivered to it.
    ///
    /// Returns `true` when an active sender was removed. A dropped receiver is
    /// cleaned up on the next publication as well.
    pub fn unsubscribe(&mut self, id: SubscriptionId) -> bool {
        self.subscriptions.remove(&id).is_some()
    }

    /// Publishes one event to every active subscriber without blocking.
    ///
    /// Each subscriber has an independent queue. If one queue is full, the
    /// method attempts every subscriber, removes disconnected receivers, and
    /// returns [`ErrorCode::QueueFull`]; no queue entry is silently discarded.
    ///
    /// # Errors
    /// Returns [`ErrorCode::QueueFull`] when a subscriber has no capacity.
    pub fn publish(&mut self, event: E) -> Result<usize>
    where
        E: Clone,
    {
        let mut delivered = 0;
        let mut disconnected = Vec::new();
        let mut backpressure = false;
        for (&id, sender) in &self.subscriptions {
            match sender.try_send(event.clone()) {
                Ok(()) => delivered += 1,
                Err(TrySendError::Full(_)) => {
                    backpressure = true;
                }
                Err(TrySendError::Disconnected(_)) => disconnected.push(id),
            }
        }
        for id in disconnected {
            self.subscriptions.remove(&id);
        }
        if backpressure {
            return Err(MetisError::transport(
                ErrorCode::QueueFull,
                "Event subscription queue is full",
            ));
        }
        Ok(delivered)
    }

    /// Returns the number of active senders.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscriptions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_input_sensitive_values_to_each_subscriber() {
        let mut hub = EventHub::<String, 2>::new().expect("positive capacity");
        let first = hub.subscribe().expect("first subscriber");
        let second = hub.subscribe().expect("second subscriber");
        assert_eq!(hub.publish("alpha".to_owned()).expect("publish"), 2);
        assert_eq!(hub.publish("beta".to_owned()).expect("publish"), 2);
        assert_eq!(
            first.recv_timeout(Duration::from_secs(1)).expect("alpha"),
            "alpha"
        );
        assert_eq!(
            first.recv_timeout(Duration::from_secs(1)).expect("beta"),
            "beta"
        );
        assert_eq!(
            second.recv_timeout(Duration::from_secs(1)).expect("alpha"),
            "alpha"
        );
        assert_eq!(
            second.recv_timeout(Duration::from_secs(1)).expect("beta"),
            "beta"
        );
    }

    #[test]
    fn full_queue_reports_backpressure_without_dropping_a_queued_event() {
        let mut hub = EventHub::<u32, 1>::new().expect("positive capacity");
        let subscription = hub.subscribe().expect("subscriber");
        assert_eq!(hub.publish(7).expect("first publish"), 1);
        let error = hub.publish(11).expect_err("full queue");
        assert_eq!(error.code, ErrorCode::QueueFull);
        assert_eq!(
            subscription
                .recv_timeout(Duration::from_secs(1))
                .expect("queued value"),
            7
        );
    }

    #[test]
    fn unsubscribe_stops_future_delivery_and_disconnects_receiver() {
        let mut hub = EventHub::<u32, 2>::new().expect("positive capacity");
        let subscription = hub.subscribe().expect("subscriber");
        let id = subscription.id();
        assert!(hub.unsubscribe(id));
        assert!(!hub.unsubscribe(id));
        assert_eq!(hub.publish(5).expect("publish without subscriber"), 0);
        assert_eq!(
            subscription
                .try_recv()
                .expect_err("unsubscribed receiver disconnects")
                .code,
            ErrorCode::ConnectionClosed
        );
    }

    #[test]
    fn disconnected_receivers_are_reclaimed_on_publish() {
        let mut hub = EventHub::<u32, 2>::new().expect("positive capacity");
        let subscription = hub.subscribe().expect("subscriber");
        drop(subscription);
        assert_eq!(hub.publish(3).expect("reclaim disconnected"), 0);
        assert_eq!(hub.subscriber_count(), 0);
    }

    #[test]
    fn full_queue_does_not_preserve_disconnected_subscribers() {
        let mut hub = EventHub::<u32, 1>::new().expect("positive capacity");
        let full = hub.subscribe().expect("full subscriber");
        let disconnected = hub.subscribe().expect("disconnected subscriber");
        assert_eq!(hub.publish(3).expect("initial publish"), 2);
        drop(disconnected);
        let error = hub.publish(5).expect_err("full queue");
        assert_eq!(error.code, ErrorCode::QueueFull);
        assert_eq!(hub.subscriber_count(), 1);
        assert_eq!(full.try_recv().expect("queued value"), Some(3));
    }

    #[test]
    fn zero_capacity_and_zero_deadline_are_rejected() {
        let error = EventHub::<u32, 0>::new().expect_err("zero capacity");
        assert_eq!(error.code, ErrorCode::QueueFull);
        let mut hub = EventHub::<u32, 1>::new().expect("positive capacity");
        let subscription = hub.subscribe().expect("subscriber");
        assert_eq!(
            subscription
                .recv_timeout(Duration::ZERO)
                .expect_err("zero deadline")
                .code,
            ErrorCode::Timeout
        );
    }

    #[test]
    fn subscription_count_is_bounded() {
        let mut hub = EventHub::<u32, 1>::new().expect("positive capacity");
        let mut subscriptions = Vec::with_capacity(MAX_SUBSCRIPTIONS);
        for _ in 0..MAX_SUBSCRIPTIONS {
            subscriptions.push(hub.subscribe().expect("subscription capacity"));
        }
        let error = hub.subscribe().expect_err("subscription bound");
        assert_eq!(error.code, ErrorCode::QueueFull);
        assert_eq!(hub.subscriber_count(), MAX_SUBSCRIPTIONS);
        drop(subscriptions);
    }
}
