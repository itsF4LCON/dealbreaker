use serde::Serialize;

use super::{
    Event, Game, MiniKind, MiniMode, Phase, PlayerId, Standing, TargetPurpose, WheelOutcome,
};
use crate::cards::{Card, Match};

#[derive(Debug, Serialize)]
pub struct PlayerView<'a> {
    pub id: PlayerId,
    pub name: &'a str,
    pub cards: usize,
    pub connected: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PhaseView {
    Lobby,
    Turn {
        player: PlayerId,
        drawn: Option<u16>,
        playable: Vec<u16>,
    },
    Target {
        player: PlayerId,
        purpose: TargetPurpose,
        options: Vec<PlayerId>,
    },
    Discard {
        player: PlayerId,
    },
    Minigame {
        id: u32,
        game: MiniKind,
        mode: MiniMode,
        participants: Vec<PlayerId>,
        submitted: Vec<PlayerId>,
        starts_in: f64,
        param: Option<u32>,
        results: Option<Vec<Standing>>,
        penalty: usize,
    },
    Wheel {
        player: PlayerId,
        outcome: WheelOutcome,
    },
    Over {
        winner: PlayerId,
    },
}

#[derive(Debug, Serialize)]
pub struct View<'a> {
    pub you: PlayerId,
    pub host: Option<PlayerId>,
    pub players: Vec<PlayerView<'a>>,
    pub hand: &'a [Card],
    pub top: Option<Card>,
    pub to_match: Option<Match>,
    pub pile: usize,
    pub direction: i8,
    pub phase: PhaseView,
    pub remaining: Option<f64>,
    pub events: &'a [Event],
}

impl Game {
    pub fn view(&self, id: PlayerId, now: f64) -> Option<View<'_>> {
        let me = self.player(id)?;
        let phase = match &self.phase {
            Phase::Lobby => PhaseView::Lobby,
            Phase::Turn { player, drawn, .. } => {
                let mine = *player == id;
                let playable = if mine {
                    me.hand
                        .iter()
                        .filter(|c| match drawn {
                            Some(d) => c.id == *d,
                            None => {
                                c.fits(self.to_match) && !(c.is_special() && me.hand.len() == 1)
                            }
                        })
                        .map(|c| c.id)
                        .collect()
                } else {
                    Vec::new()
                };
                PhaseView::Turn {
                    player: *player,
                    drawn: if mine { *drawn } else { None },
                    playable,
                }
            }
            Phase::ChooseTarget {
                player, purpose, ..
            } => PhaseView::Target {
                player: *player,
                purpose: *purpose,
                options: self.others(*player),
            },
            Phase::ChooseDiscard { player, .. } => PhaseView::Discard { player: *player },
            Phase::MiniGame(m) => PhaseView::Minigame {
                id: m.id,
                game: m.kind,
                mode: m.mode,
                participants: m.participants.clone(),
                submitted: m.submitted.iter().map(|(p, _)| *p).collect(),
                starts_in: m.play_starts - now,
                param: m.secret_for(id),
                results: m.standings.clone(),
                penalty: m.mode.penalty(),
            },
            Phase::Wheel {
                player, outcome, ..
            } => PhaseView::Wheel {
                player: *player,
                outcome: *outcome,
            },
            Phase::Over { winner } => PhaseView::Over { winner: *winner },
        };
        Some(View {
            you: id,
            host: self.host,
            players: self
                .players
                .iter()
                .map(|p| PlayerView {
                    id: p.id,
                    name: &p.name,
                    cards: p.hand.len(),
                    connected: p.connected,
                })
                .collect(),
            hand: &me.hand,
            top: self.discard.last().copied(),
            to_match: self.to_match,
            pile: self.draw_pile.len(),
            direction: self.direction,
            phase,
            remaining: self.deadline().map(|d| (d - now).max(0.0)),
            events: &self.events,
        })
    }
}
