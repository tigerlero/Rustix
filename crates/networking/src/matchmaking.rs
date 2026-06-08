//! Matchmaking and lobby API stub.
//!
//! Provides data structures and message types for a lobby-based
//! matchmaking system. The actual network transport to a matchmaking
//! server is left to the caller; this module defines the protocol
//! and in-memory lobby state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ClientId;

/// Unique identifier for a lobby.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LobbyId(pub u64);

/// Information about a player in a lobby.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub client_id: ClientId,
    pub display_name: String,
    pub ready: bool,
    pub team: Option<u8>,
}

/// A lobby that groups players before a match starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lobby {
    pub id: LobbyId,
    pub host: ClientId,
    pub name: String,
    pub max_players: u8,
    pub players: Vec<LobbyPlayer>,
    pub password_protected: bool,
    pub game_mode: String,
    pub map: String,
    pub started: bool,
}

impl Lobby {
    pub fn new(id: LobbyId, host: ClientId, name: impl Into<String>, max_players: u8) -> Self {
        Self {
            id,
            host,
            name: name.into(),
            max_players,
            players: Vec::new(),
            password_protected: false,
            game_mode: String::new(),
            map: String::new(),
            started: false,
        }
    }

    /// Number of currently joined players.
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Whether the lobby is full.
    pub fn is_full(&self) -> bool {
        self.player_count() >= self.max_players as usize
    }

    /// Whether all players are ready and there are at least two.
    pub fn can_start(&self) -> bool {
        self.players.len() >= 2 && self.players.iter().all(|p| p.ready)
    }

    /// Add a player to the lobby.
    pub fn join(&mut self, player: LobbyPlayer) -> Result<(), String> {
        if self.is_full() {
            return Err("Lobby is full".to_string());
        }
        if self.players.iter().any(|p| p.client_id == player.client_id) {
            return Err("Player already in lobby".to_string());
        }
        self.players.push(player);
        Ok(())
    }

    /// Remove a player from the lobby.
    pub fn leave(&mut self, client_id: ClientId) {
        self.players.retain(|p| p.client_id != client_id);
    }

    /// Mark a player as ready.
    pub fn set_ready(&mut self, client_id: ClientId, ready: bool) {
        if let Some(p) = self.players.iter_mut().find(|p| p.client_id == client_id) {
            p.ready = ready;
        }
    }

    /// Set the team for a player.
    pub fn set_team(&mut self, client_id: ClientId, team: u8) {
        if let Some(p) = self.players.iter_mut().find(|p| p.client_id == client_id) {
            p.team = Some(team);
        }
    }

    /// Start the match.
    pub fn start(&mut self) -> Result<(), String> {
        if self.started {
            return Err("Already started".to_string());
        }
        if !self.can_start() {
            return Err("Not all players are ready".to_string());
        }
        self.started = true;
        Ok(())
    }
}

/// Matchmaking request sent by a client to the matchmaking service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchmakingRequest {
    /// Create a new lobby and become the host.
    CreateLobby {
        name: String,
        max_players: u8,
        password: Option<String>,
        game_mode: String,
        map: String,
    },
    /// Join an existing lobby by id.
    JoinLobby {
        lobby_id: LobbyId,
        password: Option<String>,
    },
    /// Search for lobbies matching criteria.
    SearchLobbies {
        game_mode: Option<String>,
        map: Option<String>,
        not_full: bool,
        not_started: bool,
    },
    /// Leave the current lobby.
    LeaveLobby,
    /// Signal ready / not-ready.
    SetReady { ready: bool },
    /// Set team assignment.
    SetTeam { team: u8 },
    /// Host requests match start.
    StartMatch,
}

/// Matchmaking response sent by the service to a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchmakingResponse {
    /// Lobby created successfully.
    LobbyCreated { lobby: Lobby },
    /// Joined a lobby.
    JoinedLobby { lobby: Lobby },
    /// Search results.
    LobbyList { lobbies: Vec<Lobby> },
    /// Another player joined the lobby.
    PlayerJoined { player: LobbyPlayer },
    /// Another player left the lobby.
    PlayerLeft { client_id: ClientId },
    /// A player's ready state changed.
    PlayerReady { client_id: ClientId, ready: bool },
    /// Match is starting.
    MatchStarting { lobby_id: LobbyId },
    /// Generic error.
    Error { message: String },
}

/// In-memory lobby manager (server-side stub).
///
/// Can be used for local testing or as the basis for a real
/// matchmaking server.
#[derive(Debug, Clone, Default)]
pub struct LobbyManager {
    pub lobbies: HashMap<LobbyId, Lobby>,
    pub next_lobby_id: u64,
    /// Which lobby each client is currently in, if any.
    pub client_lobbies: HashMap<ClientId, LobbyId>,
}

impl LobbyManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new lobby and add the host.
    pub fn create_lobby(
        &mut self,
        host: ClientId,
        name: impl Into<String>,
        max_players: u8,
        game_mode: impl Into<String>,
        map: impl Into<String>,
    ) -> LobbyId {
        let id = LobbyId(self.next_lobby_id);
        self.next_lobby_id += 1;

        let mut lobby = Lobby::new(id, host, name, max_players);
        lobby.game_mode = game_mode.into();
        lobby.map = map.into();

        let player = LobbyPlayer {
            client_id: host,
            display_name: format!("Player{}", host.0),
            ready: false,
            team: None,
        };
        let _ = lobby.join(player);

        self.lobbies.insert(id, lobby);
        self.client_lobbies.insert(host, id);
        id
    }

    /// Remove a client from their current lobby.
    pub fn leave_lobby(&mut self, client_id: ClientId) -> Option<LobbyId> {
        let lobby_id = self.client_lobbies.remove(&client_id)?;
        if let Some(lobby) = self.lobbies.get_mut(&lobby_id) {
            lobby.leave(client_id);
            if lobby.players.is_empty() {
                self.lobbies.remove(&lobby_id);
            } else if lobby.host == client_id {
                // Transfer host to the next player.
                if let Some(new_host) = lobby.players.first().map(|p| p.client_id) {
                    lobby.host = new_host;
                }
            }
        }
        Some(lobby_id)
    }

    /// Add a client to an existing lobby.
    pub fn join_lobby(&mut self, client_id: ClientId, lobby_id: LobbyId) -> Result<(), String> {
        let lobby = self.lobbies.get_mut(&lobby_id).ok_or("Lobby not found")?;
        if lobby.started {
            return Err("Match already started".to_string());
        }
        let player = LobbyPlayer {
            client_id,
            display_name: format!("Player{}", client_id.0),
            ready: false,
            team: None,
        };
        lobby.join(player)?;
        self.client_lobbies.insert(client_id, lobby_id);
        Ok(())
    }

    /// Set a player's ready state.
    pub fn set_ready(&mut self, client_id: ClientId, ready: bool) -> Result<(), String> {
        let lobby_id = self.client_lobbies.get(&client_id).copied().ok_or("Not in a lobby")?;
        if let Some(lobby) = self.lobbies.get_mut(&lobby_id) {
            lobby.set_ready(client_id, ready);
        }
        Ok(())
    }

    /// Set a player's team.
    pub fn set_team(&mut self, client_id: ClientId, team: u8) -> Result<(), String> {
        let lobby_id = self.client_lobbies.get(&client_id).copied().ok_or("Not in a lobby")?;
        if let Some(lobby) = self.lobbies.get_mut(&lobby_id) {
            lobby.set_team(client_id, team);
        }
        Ok(())
    }

    /// Start a match if the caller is the host and all players are ready.
    pub fn start_match(&mut self, client_id: ClientId) -> Result<LobbyId, String> {
        let lobby_id = self.client_lobbies.get(&client_id).copied().ok_or("Not in a lobby")?;
        let lobby = self.lobbies.get_mut(&lobby_id).ok_or("Lobby not found")?;
        if lobby.host != client_id {
            return Err("Only the host can start".to_string());
        }
        lobby.start()?;
        Ok(lobby_id)
    }

    /// Search for lobbies matching the given criteria.
    pub fn search(&self, criteria: &MatchmakingRequest) -> Vec<Lobby> {
        let mut result = Vec::new();
        let (game_mode, map, not_full, not_started) = match criteria {
            MatchmakingRequest::SearchLobbies {
                game_mode,
                map,
                not_full,
                not_started,
            } => (game_mode.as_deref(), map.as_deref(), *not_full, *not_started),
            _ => return result,
        };

        for lobby in self.lobbies.values() {
            if not_full && lobby.is_full() {
                continue;
            }
            if not_started && lobby.started {
                continue;
            }
            if let Some(ref gm) = game_mode {
                if &lobby.game_mode != gm {
                    continue;
                }
            }
            if let Some(ref m) = map {
                if &lobby.map != m {
                    continue;
                }
            }
            result.push(lobby.clone());
        }
        result
    }

    /// Get a lobby by id.
    pub fn get(&self, lobby_id: LobbyId) -> Option<&Lobby> {
        self.lobbies.get(&lobby_id)
    }

    /// Mutable access to a lobby.
    pub fn get_mut(&mut self, lobby_id: LobbyId) -> Option<&mut Lobby> {
        self.lobbies.get_mut(&lobby_id)
    }

    /// Remove a lobby entirely.
    pub fn remove_lobby(&mut self, lobby_id: LobbyId) {
        if let Some(lobby) = self.lobbies.remove(&lobby_id) {
            for player in &lobby.players {
                self.client_lobbies.remove(&player.client_id);
            }
        }
    }

    /// Number of active lobbies.
    pub fn lobby_count(&self) -> usize {
        self.lobbies.len()
    }
}
