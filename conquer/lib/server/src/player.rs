use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Player {
    pub id: u32,
    pub name: String,
    pub level: u8,
    pub ex_cnt: u64, // Experience count
}

#[async_trait]
pub trait PlayerRepository {
    async fn get_player(&self, id: &u32) -> Option<Player>; // 根据 id 查到玩家
    async fn save_player(&mut self, player: &Player) -> bool; // 保存玩家数据
    async fn delete_player(&mut self, id: &u32) -> bool; // 删除玩家数据
    async fn list_players(&self) -> Vec<Player>; // 列出所有玩家
}

pub struct InMemoryPlayerRepository {
    players: Arc<Mutex<HashMap<u32, Player>>>,
}

impl InMemoryPlayerRepository {
    pub fn new() -> Self {
        Self {
            players: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn length(&self) -> usize {
        self.players.lock().unwrap().len()
    }
}
#[async_trait]
impl PlayerRepository for InMemoryPlayerRepository {
    async fn get_player(&self, id: &u32) -> Option<Player> {
        self.players.lock().unwrap().get(id).cloned()
    }

    async fn save_player(&mut self, player: &Player) -> bool {
        self.players
            .lock()
            .unwrap()
            .insert(player.id, player.clone());
        true
    }

    async fn delete_player(&mut self, id: &u32) -> bool {
        self.players.lock().unwrap().remove(id).is_some()
    }

    async fn list_players(&self) -> Vec<Player> {
        self.players.lock().unwrap().values().cloned().collect()
    }
}
