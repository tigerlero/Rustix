//! Tests for matchmaking and lobby API.

use crate::matchmaking::*;
use crate::ClientId;

fn player(id: u64) -> LobbyPlayer {
    LobbyPlayer {
        client_id: ClientId(id),
        display_name: format!("Player{id}"),
        ready: false,
        team: None,
    }
}

fn ready_player(id: u64) -> LobbyPlayer {
    LobbyPlayer {
        client_id: ClientId(id),
        display_name: format!("Player{id}"),
        ready: true,
        team: None,
    }
}

// ---------- Lobby ----------

#[test]
fn lobby_new() {
    let lobby = Lobby::new(LobbyId(1), ClientId(1), "Test", 4);
    assert_eq!(lobby.id, LobbyId(1));
    assert_eq!(lobby.host, ClientId(1));
    assert_eq!(lobby.name, "Test");
    assert_eq!(lobby.max_players, 4);
    assert_eq!(lobby.player_count(), 0);
    assert!(!lobby.is_full());
    assert!(!lobby.started);
}

#[test]
fn lobby_join_and_leave() {
    let mut lobby = Lobby::new(LobbyId(1), ClientId(1), "Test", 2);
    lobby.join(player(2)).unwrap();
    assert_eq!(lobby.player_count(), 1);
    lobby.join(player(3)).unwrap();
    assert_eq!(lobby.player_count(), 2);
    assert!(lobby.is_full());
    assert!(lobby.join(player(4)).is_err());
    lobby.leave(ClientId(2));
    assert_eq!(lobby.player_count(), 1);
}

#[test]
fn lobby_join_duplicate() {
    let mut lobby = Lobby::new(LobbyId(1), ClientId(1), "Test", 4);
    lobby.join(player(2)).unwrap();
    assert!(lobby.join(player(2)).is_err());
}

#[test]
fn lobby_ready_and_can_start() {
    let mut lobby = Lobby::new(LobbyId(1), ClientId(1), "Test", 4);
    lobby.join(player(1)).unwrap();
    lobby.join(player(2)).unwrap();
    assert!(!lobby.can_start());
    lobby.set_ready(ClientId(1), true);
    lobby.set_ready(ClientId(2), true);
    assert!(lobby.can_start());
}

#[test]
fn lobby_set_team() {
    let mut lobby = Lobby::new(LobbyId(1), ClientId(1), "Test", 4);
    lobby.join(player(1)).unwrap();
    lobby.set_team(ClientId(1), 1);
    assert_eq!(lobby.players[0].team, Some(1));
}

#[test]
fn lobby_start() {
    let mut lobby = Lobby::new(LobbyId(1), ClientId(1), "Test", 4);
    lobby.join(ready_player(1)).unwrap();
    lobby.join(ready_player(2)).unwrap();
    lobby.start().unwrap();
    assert!(lobby.started);
    assert!(lobby.start().is_err());
}

#[test]
fn lobby_start_not_ready() {
    let mut lobby = Lobby::new(LobbyId(1), ClientId(1), "Test", 4);
    lobby.join(player(1)).unwrap();
    lobby.join(player(2)).unwrap();
    assert!(lobby.start().is_err());
}

// ---------- LobbyManager ----------

#[test]
fn lobby_manager_new() {
    let lm = LobbyManager::new();
    assert_eq!(lm.lobby_count(), 0);
}

#[test]
fn lobby_manager_create_and_get() {
    let mut lm = LobbyManager::new();
    let id = lm.create_lobby(ClientId(1), "MyLobby", 4, "dm", "map1");
    assert_eq!(lm.lobby_count(), 1);
    let lobby = lm.get(id).unwrap();
    assert_eq!(lobby.name, "MyLobby");
    assert_eq!(lobby.game_mode, "dm");
    assert_eq!(lobby.map, "map1");
    assert_eq!(lobby.host, ClientId(1));
}

#[test]
fn lobby_manager_join_and_leave() {
    let mut lm = LobbyManager::new();
    let id = lm.create_lobby(ClientId(1), "MyLobby", 4, "dm", "map1");
    lm.join_lobby(ClientId(2), id).unwrap();
    assert_eq!(lm.get(id).unwrap().player_count(), 2);
    let left = lm.leave_lobby(ClientId(2));
    assert_eq!(left, Some(id));
    assert_eq!(lm.get(id).unwrap().player_count(), 1);
}

#[test]
fn lobby_manager_leave_transfers_host() {
    let mut lm = LobbyManager::new();
    let id = lm.create_lobby(ClientId(1), "MyLobby", 4, "dm", "map1");
    lm.join_lobby(ClientId(2), id).unwrap();
    lm.leave_lobby(ClientId(1));
    let lobby = lm.get(id).unwrap();
    assert_eq!(lobby.host, ClientId(2));
}

#[test]
fn lobby_manager_leave_removes_empty_lobby() {
    let mut lm = LobbyManager::new();
    let id = lm.create_lobby(ClientId(1), "MyLobby", 4, "dm", "map1");
    lm.leave_lobby(ClientId(1));
    assert_eq!(lm.lobby_count(), 0);
    assert!(lm.get(id).is_none());
}

#[test]
fn lobby_manager_start_match() {
    let mut lm = LobbyManager::new();
    let id = lm.create_lobby(ClientId(1), "MyLobby", 4, "dm", "map1");
    lm.join_lobby(ClientId(2), id).unwrap();
    lm.set_ready(ClientId(1), true).unwrap();
    lm.set_ready(ClientId(2), true).unwrap();
    let started = lm.start_match(ClientId(1)).unwrap();
    assert_eq!(started, id);
    assert!(lm.get(id).unwrap().started);
}

#[test]
fn lobby_manager_start_match_not_host() {
    let mut lm = LobbyManager::new();
    let id = lm.create_lobby(ClientId(1), "MyLobby", 4, "dm", "map1");
    lm.join_lobby(ClientId(2), id).unwrap();
    assert!(lm.start_match(ClientId(2)).is_err());
}

#[test]
fn lobby_manager_set_team() {
    let mut lm = LobbyManager::new();
    let id = lm.create_lobby(ClientId(1), "MyLobby", 4, "dm", "map1");
    lm.set_team(ClientId(1), 2).unwrap();
    assert_eq!(lm.get(id).unwrap().players[0].team, Some(2));
}

#[test]
fn lobby_manager_search() {
    let mut lm = LobbyManager::new();
    let id1 = lm.create_lobby(ClientId(1), "Lobby1", 4, "dm", "map1");
    let id2 = lm.create_lobby(ClientId(2), "Lobby2", 2, "ctf", "map2");
    let id3 = lm.create_lobby(ClientId(3), "Lobby3", 4, "dm", "map1");

    // Fill lobby 2 so it is full
    lm.join_lobby(ClientId(4), id2).unwrap();

    let criteria = MatchmakingRequest::SearchLobbies {
        game_mode: Some("dm".into()),
        map: Some("map1".into()),
        not_full: true,
        not_started: true,
    };
    let results = lm.search(&criteria);
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|l| l.id == id1));
    assert!(results.iter().any(|l| l.id == id3));
}

#[test]
fn lobby_manager_remove_lobby() {
    let mut lm = LobbyManager::new();
    let id = lm.create_lobby(ClientId(1), "MyLobby", 4, "dm", "map1");
    lm.join_lobby(ClientId(2), id).unwrap();
    lm.remove_lobby(id);
    assert_eq!(lm.lobby_count(), 0);
    assert!(lm.client_lobbies.get(&ClientId(1)).is_none());
    assert!(lm.client_lobbies.get(&ClientId(2)).is_none());
}

#[test]
fn lobby_manager_join_started_fails() {
    let mut lm = LobbyManager::new();
    let id = lm.create_lobby(ClientId(1), "MyLobby", 4, "dm", "map1");
    lm.join_lobby(ClientId(2), id).unwrap();
    lm.set_ready(ClientId(1), true).unwrap();
    lm.set_ready(ClientId(2), true).unwrap();
    lm.start_match(ClientId(1)).unwrap();
    assert!(lm.join_lobby(ClientId(3), id).is_err());
}

// ---------- Request / Response ----------

#[test]
fn matchmaking_request_variants() {
    let req = MatchmakingRequest::CreateLobby {
        name: "test".into(),
        max_players: 4,
        password: None,
        game_mode: "dm".into(),
        map: "map1".into(),
    };
    assert!(matches!(req, MatchmakingRequest::CreateLobby { .. }));
}

#[test]
fn matchmaking_response_variants() {
    let lobby = Lobby::new(LobbyId(1), ClientId(1), "test", 4);
    let resp = MatchmakingResponse::LobbyCreated { lobby };
    assert!(matches!(resp, MatchmakingResponse::LobbyCreated { .. }));
}
