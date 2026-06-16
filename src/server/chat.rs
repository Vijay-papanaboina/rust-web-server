use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use crate::server::websocket::{Message, WsSender, WsReceiver, WsEvent};

pub struct ChatManager {
    state: Mutex<ChatState>,
}

struct ChatState {
    chats: HashMap<usize, HashSet<usize>>,
    users: HashMap<usize, usize>,
    socket_to_chat: HashMap<usize, usize>,
    senders: HashMap<usize, WsSender>,
}

impl ChatManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(ChatState {
                chats: HashMap::new(),
                users: HashMap::new(),
                socket_to_chat: HashMap::new(),
                senders: HashMap::new(),
            }),
        }
    }

    pub fn handle_connection(self: &Arc<Self>, mut receiver: WsReceiver) {
        let chat_manager = self.clone();
        tokio::spawn(async move {
            let mut current_socket_id = None;
            let mut current_chat_id = None;

            while let Some(event) = receiver.recv().await {
                match event {
                    WsEvent::Connect { socket_id, handshake, sender } => {
                        let chat_id = handshake.query_params.get("chat_id")
                            .and_then(|s| s.parse::<usize>().ok());
                        let user_id = handshake.query_params.get("user_id")
                            .and_then(|s| s.parse::<usize>().ok());

                        if let (Some(chat_id), Some(user_id)) = (chat_id, user_id) {
                            current_socket_id = Some(socket_id);
                            current_chat_id = Some(chat_id);
                            let mut state = chat_manager.state.lock().unwrap();
                            state.chats.entry(chat_id).or_default().insert(socket_id);
                            state.users.insert(socket_id, user_id);
                            state.socket_to_chat.insert(socket_id, chat_id);
                            state.senders.insert(socket_id, sender);
                            println!("Client {} (User {}) connected to Chat {}", socket_id, user_id, chat_id);
                        } else {
                            println!("Connection rejected for socket {}: invalid parameters", socket_id);
                            break;
                        }
                    }
                    WsEvent::Message(msg) => {
                        if let (Some(socket_id), Some(chat_id)) = (current_socket_id, current_chat_id) {
                            chat_manager.send_message(chat_id, socket_id, msg);
                        }
                    }
                    WsEvent::Disconnect => {
                        if let Some(socket_id) = current_socket_id {
                            chat_manager.unregister(socket_id);
                        }
                        break;
                    }
                }
            }
        });
    }

    pub fn send_message(&self, chat_id: usize, sender_socket_id: usize, msg: Message) {
        let state = self.state.lock().unwrap();
        if let Some(members) = state.chats.get(&chat_id) {
            for &member_id in members {
                if member_id != sender_socket_id {
                    if let Some(sender) = state.senders.get(&member_id) {
                        let _ = sender.send(msg.clone());
                    }
                }
            }
        }
    }

    pub fn unregister(&self, socket_id: usize) {
        let mut state = self.state.lock().unwrap();
        println!("Client {} disconnected", socket_id);
        state.users.remove(&socket_id);
        state.senders.remove(&socket_id);
        if let Some(chat_id) = state.socket_to_chat.remove(&socket_id) {
            if let Some(members) = state.chats.get_mut(&chat_id) {
                members.remove(&socket_id);
                if members.is_empty() {
                    state.chats.remove(&chat_id);
                }
            }
        }
    }
}
