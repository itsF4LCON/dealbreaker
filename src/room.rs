use std::cell::RefCell;

use dealbreaker_engine::{Action, Game, PlayerId, View};
use serde::{Deserialize, Serialize};
use worker::*;

const GAME_KEY: &str = "game";
const ACTIVE_KEY: &str = "active";
const IDLE_MS: f64 = 30.0 * 60.0 * 1000.0;
const MAX_MESSAGE_BYTES: usize = 1024;

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
struct Seat {
    player: Option<PlayerId>,
}

#[derive(Deserialize)]
struct Join {
    name: String,
    token: String,
}

#[derive(Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum Outgoing<'a> {
    State { view: View<'a> },
    Error { message: &'a str },
    Left,
}

#[derive(Serialize)]
struct Info {
    lobby: bool,
    players: usize,
}

#[durable_object]
pub struct Room {
    state: State,
    game: RefCell<Option<Game>>,
}

impl DurableObject for Room {
    fn new(state: State, _env: Env) -> Self {
        Self { state, game: RefCell::new(None) }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let upgrade = req.headers().get("Upgrade")?.unwrap_or_default();
        if upgrade.eq_ignore_ascii_case("websocket") {
            return self.connect().await;
        }
        match (req.method(), req.path().as_str()) {
            (Method::Post, "/create") => self.create().await,
            (Method::Get, "/info") => self.info().await,
            _ => Response::error("Not found", 404),
        }
    }

    async fn websocket_message(&self, ws: WebSocket, message: WebSocketIncomingMessage) -> Result<()> {
        let WebSocketIncomingMessage::String(text) = message else {
            return Ok(());
        };
        if text.len() > MAX_MESSAGE_BYTES {
            return Ok(());
        }
        if !self.load().await? {
            let _ = ws.close(Some(4404), Some("Room not found"));
            return Ok(());
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            return Ok(());
        };
        let now = now_ms();
        let seat = seat_of(&ws);

        let outcome = {
            let mut cell = self.game.borrow_mut();
            let game = cell.as_mut().expect("game is loaded");
            game.tick(now);
            if value.get("t").and_then(|t| t.as_str()) == Some("join") {
                join(game, &ws, value, now)
            } else {
                match (seat.player, serde_json::from_value::<Action>(value)) {
                    (Some(id), Ok(Action::Leave)) => game.act(id, Action::Leave, now).map_err(|e| e.message()).map(|()| {
                        let _ = ws.serialize_attachment(Seat::default());
                        let _ = ws.send(&Outgoing::Left);
                    }),
                    (Some(id), Ok(action)) => game.act(id, action, now).map_err(|e| e.message()),
                    (None, _) => Err("Join the room first."),
                    (Some(_), Err(_)) => Err("That message wasn't understood."),
                }
            }
        };
        if let Err(message) = outcome {
            let _ = ws.send(&Outgoing::Error { message });
        }
        self.commit(now).await
    }

    async fn websocket_close(&self, ws: WebSocket, _code: usize, _reason: String, _clean: bool) -> Result<()> {
        self.disconnect(ws).await
    }

    async fn websocket_error(&self, ws: WebSocket, _error: Error) -> Result<()> {
        self.disconnect(ws).await
    }

    async fn alarm(&self) -> Result<Response> {
        if !self.load().await? {
            return Response::empty();
        }
        let now = now_ms();
        let storage = self.state.storage();
        if self.state.get_websockets().is_empty() {
            let active: f64 = storage.get(ACTIVE_KEY).await?.unwrap_or(0.0);
            if now - active >= IDLE_MS {
                storage.delete_all().await?;
                *self.game.borrow_mut() = None;
            } else {
                storage.set_alarm((active + IDLE_MS - now) as i64).await?;
            }
            return Response::empty();
        }
        if let Some(game) = self.game.borrow_mut().as_mut() {
            game.tick(now);
        }
        self.commit(now).await?;
        Response::empty()
    }
}

impl Room {
    async fn load(&self) -> Result<bool> {
        if self.game.borrow().is_some() {
            return Ok(true);
        }
        let stored: Option<String> = self.state.storage().get(GAME_KEY).await?;
        if let Some(json) = stored {
            let game: Game = serde_json::from_str(&json)?;
            self.game.borrow_mut().get_or_insert(game);
        }
        Ok(self.game.borrow().is_some())
    }

    async fn create(&self) -> Result<Response> {
        if self.load().await? {
            return Response::error("Room exists", 409);
        }
        *self.game.borrow_mut() = Some(Game::new(random_u64()?));
        self.commit(now_ms()).await?;
        Response::empty().map(|r| r.with_status(201))
    }

    async fn info(&self) -> Result<Response> {
        if !self.load().await? {
            return Response::error("Room not found", 404);
        }
        let cell = self.game.borrow();
        let game = cell.as_ref().expect("game is loaded");
        Response::from_json(&Info { lobby: game.in_lobby(), players: game.player_count() })
    }

    async fn connect(&self) -> Result<Response> {
        if !self.load().await? {
            return Response::error("Room not found", 404);
        }
        let pair = WebSocketPair::new()?;
        self.state.accept_web_socket(&pair.server);
        pair.server.serialize_attachment(Seat::default())?;
        Response::from_websocket(pair.client)
    }

    async fn disconnect(&self, ws: WebSocket) -> Result<()> {
        let _ = ws.close(Some(1000), Some("Bye"));
        if !self.load().await? {
            return Ok(());
        }
        let now = now_ms();
        if let Some(id) = seat_of(&ws).player {
            let still_here = self
                .state
                .get_websockets()
                .iter()
                .any(|other| other != &ws && seat_of(other).player == Some(id));
            if !still_here {
                if let Some(game) = self.game.borrow_mut().as_mut() {
                    game.set_connected(id, false, now);
                    game.tick(now);
                }
            }
        }
        self.commit(now).await
    }

    async fn commit(&self, now: f64) -> Result<()> {
        let sockets = self.state.get_websockets();
        let (json, deadline) = {
            let cell = self.game.borrow();
            let Some(game) = cell.as_ref() else {
                return Ok(());
            };
            for ws in &sockets {
                if let Some(view) = seat_of(ws).player.and_then(|id| game.view(id, now)) {
                    let _ = ws.send(&Outgoing::State { view });
                }
            }
            (serde_json::to_string(game)?, game.deadline())
        };
        let storage = self.state.storage();
        storage.put(GAME_KEY, json).await?;
        storage.put(ACTIVE_KEY, now).await?;
        let next = match deadline {
            Some(d) if !sockets.is_empty() => d,
            _ => now + IDLE_MS,
        };
        storage.set_alarm((next - now).max(0.0) as i64).await
    }
}

fn join(game: &mut Game, ws: &WebSocket, value: serde_json::Value, now: f64) -> std::result::Result<(), &'static str> {
    let join: Join = serde_json::from_value(value).map_err(|_| "That message wasn't understood.")?;
    if !valid_token(&join.token) {
        return Err("Your session is invalid. Refresh the page.");
    }
    let id = game.join(&join.token, &join.name).map_err(|e| e.message())?;
    ws.serialize_attachment(Seat { player: Some(id) }).map_err(|_| "Couldn't join the room.")?;
    game.set_connected(id, true, now);
    Ok(())
}

fn seat_of(ws: &WebSocket) -> Seat {
    ws.deserialize_attachment().ok().flatten().unwrap_or_default()
}

fn valid_token(token: &str) -> bool {
    (16..=64).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn now_ms() -> f64 {
    Date::now().as_millis() as f64
}

pub fn random_u64() -> Result<u64> {
    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes).map_err(|e| Error::RustError(e.to_string()))?;
    Ok(u64::from_le_bytes(bytes))
}
