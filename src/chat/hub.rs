use crate::chat::events::ServerEvent;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

/// Maximum broadcast channel capacity.
const BROADCAST_CAPACITY: usize = 1024;

/// A connected user's sender handle.
#[derive(Clone)]
pub struct UserSender {
    pub tx: broadcast::Sender<Arc<String>>,
}

/// WebSocket connection hub. Manages connected users, channel membership cache,
/// presence state, and typing throttle.
pub struct Hub {
    /// Connected users: user_id → broadcast sender (supports multiple tabs via broadcast)
    connections: RwLock<HashMap<i64, UserSender>>,
    /// Channel membership cache: channel_id → set of user_ids
    membership: RwLock<HashMap<i64, HashSet<i64>>>,
    /// Typing throttle: (user_id, channel_id) → last typing event time
    typing_throttle: RwLock<HashMap<(i64, i64), Instant>>,
}

impl Default for Hub {
    fn default() -> Self {
        Self::new()
    }
}

impl Hub {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            membership: RwLock::new(HashMap::new()),
            typing_throttle: RwLock::new(HashMap::new()),
        }
    }

    /// Register a user connection. Returns a broadcast receiver for events.
    pub async fn connect(&self, user_id: i64) -> broadcast::Receiver<Arc<String>> {
        let mut conns = self.connections.write().await;
        if let Some(sender) = conns.get(&user_id) {
            sender.tx.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(BROADCAST_CAPACITY);
            conns.insert(user_id, UserSender { tx });
            rx
        }
    }

    /// Remove a user connection.
    pub async fn disconnect(&self, user_id: i64) {
        let mut conns = self.connections.write().await;
        // Only remove if no receivers are left
        if let Some(sender) = conns.get(&user_id) {
            if sender.tx.receiver_count() <= 1 {
                conns.remove(&user_id);
            }
        }
    }

    /// Check if a user is connected (online).
    pub async fn is_online(&self, user_id: i64) -> bool {
        let conns = self.connections.read().await;
        conns.contains_key(&user_id)
    }

    /// Get all online user IDs.
    pub async fn online_users(&self) -> Vec<i64> {
        let conns = self.connections.read().await;
        conns.keys().copied().collect()
    }

    /// Set channel membership (full replace).
    pub async fn set_channel_members(&self, channel_id: i64, members: HashSet<i64>) {
        let mut mem = self.membership.write().await;
        mem.insert(channel_id, members);
    }

    /// Add a member to a channel's cached membership.
    pub async fn add_channel_member(&self, channel_id: i64, user_id: i64) {
        let mut mem = self.membership.write().await;
        mem.entry(channel_id).or_default().insert(user_id);
    }

    /// Remove a member from a channel's cached membership.
    pub async fn remove_channel_member(&self, channel_id: i64, user_id: i64) {
        let mut mem = self.membership.write().await;
        if let Some(members) = mem.get_mut(&channel_id) {
            members.remove(&user_id);
        }
    }

    /// Check if a user is in the cached membership for a channel.
    pub async fn is_channel_member(&self, channel_id: i64, user_id: i64) -> bool {
        let mem = self.membership.read().await;
        mem.get(&channel_id)
            .is_some_and(|members| members.contains(&user_id))
    }

    /// Broadcast an event to all members of a channel who are connected.
    pub async fn broadcast_to_channel(&self, channel_id: i64, event: &ServerEvent) {
        let json = match serde_json::to_string(event) {
            Ok(j) => Arc::new(j),
            Err(_) => return,
        };

        let mem = self.membership.read().await;
        let conns = self.connections.read().await;

        if let Some(members) = mem.get(&channel_id) {
            for &user_id in members {
                if let Some(sender) = conns.get(&user_id) {
                    let _ = sender.tx.send(json.clone());
                }
            }
        }
    }

    /// Broadcast an event to ALL connected users.
    pub async fn broadcast_to_all(&self, event: &ServerEvent) {
        let json = match serde_json::to_string(event) {
            Ok(j) => Arc::new(j),
            Err(_) => return,
        };
        let conns = self.connections.read().await;
        for sender in conns.values() {
            let _ = sender.tx.send(json.clone());
        }
    }

    /// Send an event to a specific user.
    pub async fn send_to_user(&self, user_id: i64, event: &ServerEvent) {
        let json = match serde_json::to_string(event) {
            Ok(j) => Arc::new(j),
            Err(_) => return,
        };

        let conns = self.connections.read().await;
        if let Some(sender) = conns.get(&user_id) {
            let _ = sender.tx.send(json);
        }
    }

    /// Check typing throttle. Returns true if typing event should be broadcast.
    /// Enforces max 1 typing event per user per channel per 3 seconds.
    pub async fn should_broadcast_typing(&self, user_id: i64, channel_id: i64) -> bool {
        let now = Instant::now();
        let key = (user_id, channel_id);

        let mut throttle = self.typing_throttle.write().await;
        if let Some(last) = throttle.get(&key) {
            if now.duration_since(*last).as_secs() < 3 {
                return false;
            }
        }
        throttle.insert(key, now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_and_disconnect() {
        let hub = Hub::new();
        let _rx = hub.connect(1).await;
        assert!(hub.is_online(1).await);
        hub.disconnect(1).await;
        assert!(!hub.is_online(1).await);
    }

    #[tokio::test]
    async fn test_channel_membership() {
        let hub = Hub::new();
        hub.set_channel_members(10, HashSet::from([1, 2, 3])).await;
        hub.add_channel_member(10, 4).await;
        hub.remove_channel_member(10, 2).await;

        let mem = hub.membership.read().await;
        let members = mem.get(&10).unwrap();
        assert!(members.contains(&1));
        assert!(!members.contains(&2));
        assert!(members.contains(&3));
        assert!(members.contains(&4));
    }

    #[tokio::test]
    async fn test_broadcast_to_channel() {
        let hub = Hub::new();
        let mut rx1 = hub.connect(1).await;
        let mut rx2 = hub.connect(2).await;
        let _rx3 = hub.connect(3).await; // not in channel

        hub.set_channel_members(10, HashSet::from([1, 2])).await;

        let event = ServerEvent::Message {
            channel: "10".into(),
            user: "1".into(),
            text: "hello".into(),
            ts: "123".into(),
            thread_ts: None,
            files: None,
        };
        hub.broadcast_to_channel(10, &event).await;

        // User 1 and 2 should receive
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_typing_throttle() {
        let hub = Hub::new();
        assert!(hub.should_broadcast_typing(1, 10).await);
        assert!(!hub.should_broadcast_typing(1, 10).await); // too soon
        assert!(hub.should_broadcast_typing(1, 20).await); // different channel OK
        assert!(hub.should_broadcast_typing(2, 10).await); // different user OK
    }
}
