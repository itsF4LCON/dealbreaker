mod minigame;
#[cfg(test)]
mod tests;
mod view;
mod wheel;

use serde::{Deserialize, Serialize};

use crate::cards::{build_deck, Card, Face, Match, CARDS_PER_DECK};
use crate::rng::Rng;
pub use minigame::{MiniGame, MiniKind, MiniMode, Standing, COUNTDOWN_MS, READY_MS, RESULTS_MS};
pub use view::{PhaseView, PlayerView, View};
pub use wheel::WheelOutcome;

pub type PlayerId = u32;

pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 8;
pub const HAND_SIZE: usize = 7;
pub const MAX_NAME_CHARS: usize = 16;
pub const TURN_MS: f64 = 30_000.0;
pub const DRAWN_MS: f64 = 15_000.0;
pub const OFFLINE_TURN_MS: f64 = 5_000.0;
pub const CHOOSE_MS: f64 = 15_000.0;
pub const SPIN_MS: f64 = 4_500.0;
pub const WHEEL_WAIT_MS: f64 = 15_000.0;
const MAX_EVENTS: usize = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameError {
    NameInvalid,
    RoomFull,
    InProgress,
    NotHost,
    NotEnoughPlayers,
    NotYourTurn,
    WrongPhase,
    NoSuchCard,
    CardDoesNotFit,
    SpecialLastCard,
    OnlyDrawnCard,
    InvalidTarget,
    InvalidResult,
    UnknownPlayer,
}

impl GameError {
    pub fn message(self) -> &'static str {
        match self {
            GameError::NameInvalid => "Pick a name between 1 and 16 characters.",
            GameError::RoomFull => "This room is full.",
            GameError::InProgress => "This game has already started. Wait for the next one.",
            GameError::NotHost => "Only the host can do that.",
            GameError::NotEnoughPlayers => "You need at least 2 players to start.",
            GameError::NotYourTurn => "It's not your turn.",
            GameError::WrongPhase => "You can't do that right now.",
            GameError::NoSuchCard => "That card isn't in your hand.",
            GameError::CardDoesNotFit => "That card doesn't match the suit or the rank.",
            GameError::SpecialLastCard => "Your last card has to be a normal card.",
            GameError::OnlyDrawnCard => "You can only play the card you just drew, or pass.",
            GameError::InvalidTarget => "Pick another player.",
            GameError::InvalidResult => "That result isn't valid.",
            GameError::UnknownPlayer => "Join the room first.",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    token: String,
    hand: Vec<Card>,
    pub connected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetPurpose {
    Duel,
    WheelDraw,
    WheelSwap,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Phase {
    Lobby,
    Turn {
        player: PlayerId,
        drawn: Option<u16>,
        deadline: f64,
    },
    ChooseTarget {
        player: PlayerId,
        purpose: TargetPurpose,
        deadline: f64,
    },
    ChooseDiscard {
        player: PlayerId,
        deadline: f64,
    },
    MiniGame(MiniGame),
    Wheel {
        player: PlayerId,
        outcome: Option<WheelOutcome>,
        deadline: f64,
    },
    Over {
        winner: PlayerId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub seq: u32,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Action {
    Start,
    Play { card: u16 },
    Draw,
    Pass,
    Target { player: PlayerId },
    Discard { card: u16 },
    Result { value: Option<u32> },
    Ready,
    Spin,
    Again,
    Leave,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Game {
    players: Vec<Player>,
    host: Option<PlayerId>,
    next_id: PlayerId,
    phase: Phase,
    draw_pile: Vec<Card>,
    discard: Vec<Card>,
    to_match: Option<Match>,
    direction: i8,
    events: Vec<Event>,
    event_seq: u32,
    minigame_seq: u32,
    #[serde(default)]
    last_minigame: Option<MiniKind>,
    rng: Rng,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Self {
            players: Vec::new(),
            host: None,
            next_id: 1,
            phase: Phase::Lobby,
            draw_pile: Vec::new(),
            discard: Vec::new(),
            to_match: None,
            direction: 1,
            events: Vec::new(),
            event_seq: 0,
            minigame_seq: 0,
            last_minigame: None,
            rng: Rng::new(seed),
        }
    }

    pub fn in_lobby(&self) -> bool {
        matches!(self.phase, Phase::Lobby)
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn join(&mut self, token: &str, name: &str) -> Result<PlayerId, GameError> {
        if let Some(p) = self.players.iter().find(|p| p.token == token) {
            return Ok(p.id);
        }
        if !self.in_lobby() {
            return Err(GameError::InProgress);
        }
        if self.players.len() >= MAX_PLAYERS {
            return Err(GameError::RoomFull);
        }
        let name = clean_name(name).ok_or(GameError::NameInvalid)?;
        let id = self.next_id;
        self.next_id += 1;
        self.log(format!("{name} joined"));
        self.players.push(Player {
            id,
            name,
            token: token.to_string(),
            hand: Vec::new(),
            connected: false,
        });
        if self.host.is_none() {
            self.host = Some(id);
        }
        Ok(id)
    }

    pub fn set_connected(&mut self, id: PlayerId, connected: bool, now: f64) {
        match self.player_mut(id) {
            Some(p) if p.connected != connected => p.connected = connected,
            _ => return,
        }
        let host_online = self
            .host
            .and_then(|h| self.player(h))
            .is_some_and(|p| p.connected);
        if !host_online {
            if let Some(p) = self.players.iter().find(|p| p.connected) {
                self.host = Some(p.id);
            }
        }
        if !connected {
            let short = now + OFFLINE_TURN_MS;
            match &mut self.phase {
                Phase::Turn {
                    player, deadline, ..
                }
                | Phase::ChooseTarget {
                    player, deadline, ..
                }
                | Phase::ChooseDiscard { player, deadline }
                | Phase::Wheel {
                    player,
                    outcome: None,
                    deadline,
                } if *player == id => {
                    *deadline = deadline.min(short);
                }
                _ => {}
            }
            self.begin_minigame_if_ready(now);
            self.finish_minigame_if_complete(now);
        }
    }

    pub fn act(&mut self, id: PlayerId, action: Action, now: f64) -> Result<(), GameError> {
        if self.player(id).is_none() {
            return Err(GameError::UnknownPlayer);
        }
        match action {
            Action::Start => self.start(id, now),
            Action::Play { card } => self.play(id, card, now),
            Action::Draw => self.draw_action(id, now),
            Action::Pass => self.pass(id, now),
            Action::Target { player } => self.choose_target(id, player, now),
            Action::Discard { card } => self.throw_away(id, card, now),
            Action::Result { value } => self.submit_result(id, value, now),
            Action::Ready => self.ready(id, now),
            Action::Spin => self.spin(id, now),
            Action::Again => self.again(id),
            Action::Leave => self.leave(id),
        }
    }

    pub fn deadline(&self) -> Option<f64> {
        match &self.phase {
            Phase::Lobby | Phase::Over { .. } => None,
            Phase::Turn { deadline, .. }
            | Phase::ChooseTarget { deadline, .. }
            | Phase::ChooseDiscard { deadline, .. }
            | Phase::Wheel { deadline, .. } => Some(*deadline),
            Phase::MiniGame(m) => Some(m.deadline),
        }
    }

    pub fn tick(&mut self, now: f64) {
        if self.deadline().is_some_and(|d| now >= d) {
            self.expire(now);
        }
    }

    fn expire(&mut self, now: f64) {
        match self.phase.clone() {
            Phase::Turn {
                player,
                drawn: None,
                ..
            } => {
                let name = self.name(player);
                self.log(format!("{name} ran out of time and drew a card"));
                self.draw(player, 1);
                self.advance(player, false, now);
            }
            Phase::Turn {
                player,
                drawn: Some(_),
                ..
            }
            | Phase::ChooseDiscard { player, .. } => {
                self.advance(player, false, now);
            }
            Phase::ChooseTarget {
                player, purpose, ..
            } => {
                let others = self.others(player);
                let target = others[self.rng.below(others.len())];
                self.apply_target(player, purpose, target, now);
            }
            Phase::MiniGame(m) if !m.started() => {
                if let Phase::MiniGame(m) = &mut self.phase {
                    m.begin(now);
                }
            }
            Phase::MiniGame(m) if m.standings.is_none() => self.resolve_minigame(now),
            Phase::MiniGame(m) => self.advance(m.mode.card_player(), false, now),
            Phase::Wheel {
                player,
                outcome: None,
                ..
            } => self.spin_wheel(player, now),
            Phase::Wheel {
                player,
                outcome: Some(outcome),
                ..
            } => self.apply_wheel(player, outcome, now),
            Phase::Lobby | Phase::Over { .. } => {}
        }
    }

    fn start(&mut self, id: PlayerId, now: f64) -> Result<(), GameError> {
        if !self.in_lobby() {
            return Err(GameError::WrongPhase);
        }
        if self.host != Some(id) {
            return Err(GameError::NotHost);
        }
        if self.players.iter().filter(|p| p.connected).count() < MIN_PLAYERS {
            return Err(GameError::NotEnoughPlayers);
        }
        self.players.retain(|p| p.connected);
        let decks = if self.players.len() <= 4 { 1 } else { 2 };
        let mut deck = build_deck(decks);
        debug_assert_eq!(deck.len(), decks * CARDS_PER_DECK);
        self.rng.shuffle(&mut deck);
        self.draw_pile = deck;
        self.discard.clear();
        self.direction = 1;
        for i in 0..self.players.len() {
            let hand = self.draw_pile.split_off(self.draw_pile.len() - HAND_SIZE);
            self.players[i].hand = hand;
        }
        loop {
            let card = self.draw_pile.pop().expect("deck has normal cards");
            if card.is_special() {
                let at = self.rng.below(self.draw_pile.len());
                self.draw_pile.insert(at, card);
                continue;
            }
            self.to_match = card.as_match();
            self.discard.push(card);
            break;
        }
        self.log("The game started".into());
        let first = self.players[self.rng.below(self.players.len())].id;
        self.begin_turn(first, now);
        Ok(())
    }

    fn play(&mut self, id: PlayerId, card_id: u16, now: f64) -> Result<(), GameError> {
        let drawn = match self.phase {
            Phase::Turn { player, drawn, .. } if player == id => drawn,
            Phase::Turn { .. } => return Err(GameError::NotYourTurn),
            _ => return Err(GameError::WrongPhase),
        };
        let to_match = self.to_match;
        let hand = &self.player(id).expect("player exists").hand;
        let index = hand
            .iter()
            .position(|c| c.id == card_id)
            .ok_or(GameError::NoSuchCard)?;
        let card = hand[index];
        if drawn.is_some_and(|d| d != card_id) {
            return Err(GameError::OnlyDrawnCard);
        }
        if card.is_special() && hand.len() == 1 {
            return Err(GameError::SpecialLastCard);
        }
        if !card.fits(to_match) {
            return Err(GameError::CardDoesNotFit);
        }

        self.hand_mut(id).remove(index);
        self.discard.push(card);
        let name = self.name(id);
        match card.face {
            Face::Normal { .. } => {
                self.to_match = card.as_match();
                if self.hand(id).is_empty() {
                    self.log(format!("{name} wins!"));
                    self.phase = Phase::Over { winner: id };
                } else {
                    self.advance(id, false, now);
                }
            }
            Face::Duel => {
                self.log(format!("{name} played a Duel card"));
                self.start_target(id, TargetPurpose::Duel, now);
            }
            Face::Party => {
                self.log(format!("{name} played a Party card"));
                let everyone = self.players.iter().map(|p| p.id).collect();
                self.start_minigame(MiniMode::Party { card_player: id }, everyone, now);
            }
            Face::Wheel => {
                self.log(format!("{name} played a Wheel card"));
                let wait = if self.player(id).is_some_and(|p| p.connected) {
                    WHEEL_WAIT_MS
                } else {
                    OFFLINE_TURN_MS
                };
                self.phase = Phase::Wheel {
                    player: id,
                    outcome: None,
                    deadline: now + wait,
                };
            }
        }
        Ok(())
    }

    fn draw_action(&mut self, id: PlayerId, now: f64) -> Result<(), GameError> {
        match self.phase {
            Phase::Turn {
                player,
                drawn: None,
                ..
            } if player == id => {}
            Phase::Turn { player, .. } if player != id => return Err(GameError::NotYourTurn),
            _ => return Err(GameError::WrongPhase),
        }
        match self.draw(id, 1).first() {
            Some(card) if card.fits(self.to_match) => {
                self.phase = Phase::Turn {
                    player: id,
                    drawn: Some(card.id),
                    deadline: now + DRAWN_MS,
                };
            }
            _ => self.advance(id, false, now),
        }
        Ok(())
    }

    fn pass(&mut self, id: PlayerId, now: f64) -> Result<(), GameError> {
        match self.phase {
            Phase::Turn {
                player,
                drawn: Some(_),
                ..
            } if player == id => {
                self.advance(id, false, now);
                Ok(())
            }
            Phase::Turn { player, .. } if player != id => Err(GameError::NotYourTurn),
            _ => Err(GameError::WrongPhase),
        }
    }

    fn start_target(&mut self, id: PlayerId, purpose: TargetPurpose, now: f64) {
        let others = self.others(id);
        if others.len() == 1 {
            self.apply_target(id, purpose, others[0], now);
        } else {
            let deadline = now + self.choice_time(id);
            self.phase = Phase::ChooseTarget {
                player: id,
                purpose,
                deadline,
            };
        }
    }

    fn choose_target(&mut self, id: PlayerId, target: PlayerId, now: f64) -> Result<(), GameError> {
        let purpose = match self.phase {
            Phase::ChooseTarget {
                player, purpose, ..
            } if player == id => purpose,
            _ => return Err(GameError::WrongPhase),
        };
        if target == id || self.player(target).is_none() {
            return Err(GameError::InvalidTarget);
        }
        self.apply_target(id, purpose, target, now);
        Ok(())
    }

    fn apply_target(&mut self, id: PlayerId, purpose: TargetPurpose, target: PlayerId, now: f64) {
        let (name, target_name) = (self.name(id), self.name(target));
        match purpose {
            TargetPurpose::Duel => {
                self.start_minigame(
                    MiniMode::Duel {
                        challenger: id,
                        target,
                    },
                    vec![id, target],
                    now,
                );
            }
            TargetPurpose::WheelDraw => {
                self.draw(target, 2);
                self.log(format!("{name} made {target_name} draw 2"));
                self.advance(id, false, now);
            }
            TargetPurpose::WheelSwap => {
                let mine = std::mem::take(self.hand_mut(id));
                let theirs = std::mem::replace(self.hand_mut(target), mine);
                *self.hand_mut(id) = theirs;
                self.log(format!("{name} swapped hands with {target_name}"));
                self.advance(id, false, now);
            }
        }
    }

    fn start_minigame(&mut self, mode: MiniMode, participants: Vec<PlayerId>, now: f64) {
        self.minigame_seq += 1;
        let options: Vec<MiniKind> = MiniKind::ALL
            .iter()
            .copied()
            .filter(|&k| Some(k) != self.last_minigame)
            .collect();
        let kind = options[self.rng.below(options.len())];
        self.last_minigame = Some(kind);
        let text = match mode {
            MiniMode::Duel { challenger, target } => {
                format!(
                    "Duel: {} vs {} in {}",
                    self.name(challenger),
                    self.name(target),
                    kind.title()
                )
            }
            MiniMode::Party { .. } => format!("Party game: {}", kind.title()),
        };
        self.log(text);
        let game = MiniGame::new(
            self.minigame_seq,
            kind,
            mode,
            participants,
            &mut self.rng,
            now,
        );
        self.phase = Phase::MiniGame(game);
        self.begin_minigame_if_ready(now);
    }

    fn ready(&mut self, id: PlayerId, now: f64) -> Result<(), GameError> {
        let Phase::MiniGame(m) = &mut self.phase else {
            return Err(GameError::WrongPhase);
        };
        if m.started() || !m.participants.contains(&id) || m.ready.contains(&id) {
            return Err(GameError::WrongPhase);
        }
        m.ready.push(id);
        self.begin_minigame_if_ready(now);
        Ok(())
    }

    fn begin_minigame_if_ready(&mut self, now: f64) {
        let Phase::MiniGame(m) = &self.phase else {
            return;
        };
        if m.started() {
            return;
        }
        let waiting = m
            .participants
            .iter()
            .any(|&p| !m.ready.contains(&p) && self.player(p).is_some_and(|pl| pl.connected));
        if !waiting {
            if let Phase::MiniGame(m) = &mut self.phase {
                m.begin(now);
            }
        }
    }

    fn spin(&mut self, id: PlayerId, now: f64) -> Result<(), GameError> {
        match self.phase {
            Phase::Wheel {
                player,
                outcome: None,
                ..
            } if player == id => {
                self.spin_wheel(id, now);
                Ok(())
            }
            Phase::Wheel { outcome: None, .. } => Err(GameError::NotYourTurn),
            _ => Err(GameError::WrongPhase),
        }
    }

    fn spin_wheel(&mut self, id: PlayerId, now: f64) {
        let outcome = WheelOutcome::ALL[self.rng.below(WheelOutcome::ALL.len())];
        let name = self.name(id);
        self.log(format!("{name} spins the wheel"));
        self.phase = Phase::Wheel {
            player: id,
            outcome: Some(outcome),
            deadline: now + SPIN_MS,
        };
    }

    fn submit_result(
        &mut self,
        id: PlayerId,
        value: Option<u32>,
        now: f64,
    ) -> Result<(), GameError> {
        let Phase::MiniGame(m) = &mut self.phase else {
            return Err(GameError::WrongPhase);
        };
        if !m.started()
            || m.standings.is_some()
            || !m.participants.contains(&id)
            || m.has_submitted(id)
        {
            return Err(GameError::WrongPhase);
        }
        if !m.accepts(value) {
            return Err(GameError::InvalidResult);
        }
        let value = m.normalize(value);
        m.submitted.push((id, value));
        self.finish_minigame_if_complete(now);
        Ok(())
    }

    fn finish_minigame_if_complete(&mut self, now: f64) {
        let Phase::MiniGame(m) = &self.phase else {
            return;
        };
        if !m.started() || m.standings.is_some() {
            return;
        }
        let waiting = m
            .participants
            .iter()
            .any(|&p| !m.has_submitted(p) && self.player(p).is_some_and(|pl| pl.connected));
        if !waiting {
            self.resolve_minigame(now);
        }
    }

    fn resolve_minigame(&mut self, now: f64) {
        let Phase::MiniGame(mut m) = std::mem::replace(&mut self.phase, Phase::Lobby) else {
            unreachable!("resolve_minigame is only called during a mini-game");
        };
        let standings = m.rank(&mut self.rng);
        let penalty = m.mode.penalty();
        for s in standings.iter().filter(|s| s.loser) {
            self.draw(s.player, penalty);
            let name = self.name(s.player);
            self.log(format!("{name} lost and drew {penalty}"));
        }
        m.standings = Some(standings);
        m.deadline = now + RESULTS_MS;
        self.phase = Phase::MiniGame(m);
    }

    fn apply_wheel(&mut self, id: PlayerId, outcome: WheelOutcome, now: f64) {
        let name = self.name(id);
        match outcome {
            WheelOutcome::EveryoneDraws => {
                for p in self.players.iter().map(|p| p.id).collect::<Vec<_>>() {
                    self.draw(p, 1);
                }
                self.log("Wheel: everyone draws 1".into());
                self.advance(id, false, now);
            }
            WheelOutcome::YouDraw => {
                self.draw(id, 2);
                self.log(format!("Wheel: {name} draws 2"));
                self.advance(id, false, now);
            }
            WheelOutcome::PickDraw => self.start_target(id, TargetPurpose::WheelDraw, now),
            WheelOutcome::SwapHands => self.start_target(id, TargetPurpose::WheelSwap, now),
            WheelOutcome::SkipNext => {
                let skipped = self.name(self.step(id, 1));
                self.log(format!("Wheel: {skipped} is skipped"));
                self.advance(id, true, now);
            }
            WheelOutcome::Reverse => {
                self.direction = -self.direction;
                self.log("Wheel: the direction is reversed".into());
                self.advance(id, false, now);
            }
            WheelOutcome::PassHands => {
                let len = self.players.len();
                let hands: Vec<Vec<Card>> = self
                    .players
                    .iter_mut()
                    .map(|p| std::mem::take(&mut p.hand))
                    .collect();
                for (i, hand) in hands.into_iter().enumerate() {
                    let to = (i as i64 + i64::from(self.direction)).rem_euclid(len as i64) as usize;
                    self.players[to].hand = hand;
                }
                self.log("Wheel: everyone passes their hand on".into());
                self.advance(id, false, now);
            }
            WheelOutcome::ThrowAway => {
                if self.hand(id).len() > 1 {
                    let deadline = now + self.choice_time(id);
                    self.phase = Phase::ChooseDiscard {
                        player: id,
                        deadline,
                    };
                } else {
                    self.advance(id, false, now);
                }
            }
        }
    }

    fn throw_away(&mut self, id: PlayerId, card_id: u16, now: f64) -> Result<(), GameError> {
        match self.phase {
            Phase::ChooseDiscard { player, .. } if player == id => {}
            _ => return Err(GameError::WrongPhase),
        }
        let hand = self.hand(id);
        let index = hand
            .iter()
            .position(|c| c.id == card_id)
            .ok_or(GameError::NoSuchCard)?;
        let card = self.hand_mut(id).remove(index);
        self.discard.insert(0, card);
        let name = self.name(id);
        self.log(format!("{name} threw a card away"));
        self.advance(id, false, now);
        Ok(())
    }

    fn again(&mut self, id: PlayerId) -> Result<(), GameError> {
        if !matches!(self.phase, Phase::Over { .. }) {
            return Err(GameError::WrongPhase);
        }
        if self.host != Some(id) {
            return Err(GameError::NotHost);
        }
        self.players.retain(|p| p.connected);
        for p in &mut self.players {
            p.hand.clear();
        }
        self.draw_pile.clear();
        self.discard.clear();
        self.to_match = None;
        self.direction = 1;
        self.phase = Phase::Lobby;
        Ok(())
    }

    fn leave(&mut self, id: PlayerId) -> Result<(), GameError> {
        if !self.in_lobby() {
            return Err(GameError::WrongPhase);
        }
        let name = self.name(id);
        self.players.retain(|p| p.id != id);
        if self.host == Some(id) {
            self.host = self
                .players
                .iter()
                .find(|p| p.connected)
                .or(self.players.first())
                .map(|p| p.id);
        }
        self.log(format!("{name} left"));
        Ok(())
    }

    fn draw(&mut self, id: PlayerId, n: usize) -> Vec<Card> {
        let mut drawn = Vec::with_capacity(n);
        for _ in 0..n {
            if self.draw_pile.is_empty() {
                self.refill();
            }
            let Some(card) = self.draw_pile.pop() else {
                break;
            };
            drawn.push(card);
        }
        self.hand_mut(id).extend(drawn.iter().copied());
        drawn
    }

    fn refill(&mut self) {
        let Some(top) = self.discard.pop() else {
            return;
        };
        self.draw_pile.append(&mut self.discard);
        self.rng.shuffle(&mut self.draw_pile);
        self.discard.push(top);
    }

    fn advance(&mut self, from: PlayerId, skip: bool, now: f64) {
        let next = self.step(from, if skip { 2 } else { 1 });
        self.begin_turn(next, now);
    }

    fn begin_turn(&mut self, id: PlayerId, now: f64) {
        let online = self.player(id).is_some_and(|p| p.connected);
        let deadline = now + if online { TURN_MS } else { OFFLINE_TURN_MS };
        self.phase = Phase::Turn {
            player: id,
            drawn: None,
            deadline,
        };
    }

    fn choice_time(&self, id: PlayerId) -> f64 {
        if self.player(id).is_some_and(|p| p.connected) {
            CHOOSE_MS
        } else {
            OFFLINE_TURN_MS
        }
    }

    fn step(&self, from: PlayerId, n: i64) -> PlayerId {
        let len = self.players.len() as i64;
        let index = self.players.iter().position(|p| p.id == from).unwrap_or(0) as i64;
        let next = (index + i64::from(self.direction) * n).rem_euclid(len);
        self.players[next as usize].id
    }

    fn others(&self, id: PlayerId) -> Vec<PlayerId> {
        self.players
            .iter()
            .map(|p| p.id)
            .filter(|&p| p != id)
            .collect()
    }

    fn player(&self, id: PlayerId) -> Option<&Player> {
        self.players.iter().find(|p| p.id == id)
    }

    fn player_mut(&mut self, id: PlayerId) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    fn hand(&self, id: PlayerId) -> &Vec<Card> {
        &self.player(id).expect("player exists").hand
    }

    fn hand_mut(&mut self, id: PlayerId) -> &mut Vec<Card> {
        &mut self.player_mut(id).expect("player exists").hand
    }

    fn name(&self, id: PlayerId) -> String {
        self.player(id).map(|p| p.name.clone()).unwrap_or_default()
    }

    fn log(&mut self, text: String) {
        self.event_seq += 1;
        self.events.push(Event {
            seq: self.event_seq,
            text,
        });
        if self.events.len() > MAX_EVENTS {
            self.events.remove(0);
        }
    }
}

fn clean_name(raw: &str) -> Option<String> {
    let name = raw
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>();
    let len = name.chars().count();
    (1..=MAX_NAME_CHARS).contains(&len).then_some(name)
}
