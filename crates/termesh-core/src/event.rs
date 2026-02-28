//! Event bus for inter-crate communication.

use crate::types::{AgentState, PaneId, SessionId};
use std::path::PathBuf;
use tokio::sync::broadcast;

/// Events that flow through the termesh event bus.
#[derive(Debug, Clone)]
pub enum Event {
    /// A new terminal session was created.
    SessionCreated {
        session_id: SessionId,
        pane_id: PaneId,
    },

    /// A terminal session was closed.
    SessionClosed { session_id: SessionId },

    /// A file was changed by an agent.
    FileChanged {
        session_id: SessionId,
        path: PathBuf,
    },

    /// An agent's state changed.
    AgentStateChanged {
        session_id: SessionId,
        old_state: AgentState,
        new_state: AgentState,
    },

    /// Pane focus changed.
    PaneFocused { pane_id: PaneId },

    /// View mode toggled (Focus <-> Split).
    ViewModeToggled,

    /// Request to shut down the application.
    Shutdown,
}

/// Broadcast-based event bus for pub/sub communication.
///
/// Uses a tokio broadcast channel so multiple subscribers can
/// independently receive all events.
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    /// Create a new event bus with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers.
    ///
    /// Returns the number of active receivers, or 0 if none.
    pub fn publish(&self, event: Event) -> usize {
        self.sender.send(event).unwrap_or(0)
    }

    /// Subscribe to the event bus. Returns a receiver that gets
    /// a copy of every event published after subscription.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    /// Returns the current number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_and_subscribe() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        bus.publish(Event::SessionCreated {
            session_id: SessionId(1),
            pane_id: PaneId(1),
        });

        let event = rx.recv().await.unwrap();
        match event {
            Event::SessionCreated {
                session_id,
                pane_id,
            } => {
                assert_eq!(session_id, SessionId(1));
                assert_eq!(pane_id, PaneId(1));
            }
            _ => panic!("unexpected event"),
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        bus.publish(Event::Shutdown);

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();

        assert!(matches!(e1, Event::Shutdown));
        assert!(matches!(e2, Event::Shutdown));
    }

    #[test]
    fn test_publish_without_subscribers() {
        let bus = EventBus::new(16);
        // Should not panic even without subscribers
        let count = bus.publish(Event::Shutdown);
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_agent_state_changed_event() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe();

        bus.publish(Event::AgentStateChanged {
            session_id: SessionId(1),
            old_state: AgentState::Idle,
            new_state: AgentState::Thinking,
        });

        let event = rx.recv().await.unwrap();
        match event {
            Event::AgentStateChanged {
                new_state,
                old_state,
                ..
            } => {
                assert_eq!(old_state, AgentState::Idle);
                assert_eq!(new_state, AgentState::Thinking);
            }
            _ => panic!("unexpected event"),
        }
    }
}
