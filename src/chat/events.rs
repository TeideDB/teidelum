use serde::{Deserialize, Serialize};

/// Events sent FROM server TO client over WebSocket.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerEvent {
    #[serde(rename = "hello")]
    Hello,

    #[serde(rename = "message")]
    Message {
        channel: String,
        user: String,
        text: String,
        ts: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thread_ts: Option<String>,
    },

    #[serde(rename = "message_changed")]
    MessageChanged {
        channel: String,
        message: MessagePayload,
    },

    #[serde(rename = "message_deleted")]
    MessageDeleted { channel: String, ts: String },

    #[serde(rename = "reaction_added")]
    ReactionAdded {
        channel: String,
        user: String,
        reaction: String,
        item_ts: String,
    },

    #[serde(rename = "reaction_removed")]
    ReactionRemoved {
        channel: String,
        user: String,
        reaction: String,
        item_ts: String,
    },

    #[serde(rename = "typing")]
    Typing { channel: String, user: String },

    #[serde(rename = "presence_change")]
    PresenceChange { user: String, presence: String },

    #[serde(rename = "member_joined_channel")]
    MemberJoinedChannel { channel: String, user: String },

    #[serde(rename = "member_left_channel")]
    MemberLeftChannel { channel: String, user: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct MessagePayload {
    pub user: String,
    pub text: String,
    pub ts: String,
    pub edited_ts: String,
}

/// Events sent FROM client TO server over WebSocket.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientEvent {
    #[serde(rename = "typing")]
    Typing { channel: String },

    #[serde(rename = "ping")]
    Ping,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_event_serialization() {
        let event = ServerEvent::Message {
            channel: "5".into(),
            user: "3".into(),
            text: "hello".into(),
            ts: "1710000000".into(),
            thread_ts: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"message\""));
        assert!(json.contains("\"text\":\"hello\""));
        // thread_ts should be absent when None
        assert!(!json.contains("thread_ts"));
    }

    #[test]
    fn test_client_event_deserialization() {
        let json = r#"{"type": "typing", "channel": "5"}"#;
        let event: ClientEvent = serde_json::from_str(json).unwrap();
        match event {
            ClientEvent::Typing { channel } => assert_eq!(channel, "5"),
            _ => panic!("expected Typing event"),
        }
    }
}
