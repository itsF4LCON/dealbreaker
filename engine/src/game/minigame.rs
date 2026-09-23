use serde::{Deserialize, Serialize};

use super::PlayerId;
use crate::rng::Rng;

pub const READY_MS: f64 = 20_000.0;
pub const COUNTDOWN_MS: f64 = 3_000.0;
pub const RESULTS_MS: f64 = 5_000.0;
pub const STOP_CLOCK_TARGET_MS: u32 = 5_000;
const GRACE_MS: f64 = 2_500.0;
const QUICK_DRAW_MIN_DELAY_MS: u32 = 1_500;
const QUICK_DRAW_MAX_DELAY_MS: u32 = 4_500;
const CASH_OUT_MIN_CRASH_MS: u32 = 2_000;
const CASH_OUT_MAX_CRASH_MS: u32 = 9_000;
const CASH_OUT_DOUBLING_MS: f64 = 3_000.0;
const HIGH_CARD_LOWEST: u32 = 2;
const HIGH_CARD_HIGHEST: u32 = 14;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiniKind {
    StopClock,
    QuickDraw,
    Mash,
    HighCard,
    CashOut,
    Stacker,
}

impl MiniKind {
    pub const ALL: [MiniKind; 6] = [
        MiniKind::StopClock,
        MiniKind::QuickDraw,
        MiniKind::Mash,
        MiniKind::HighCard,
        MiniKind::CashOut,
        MiniKind::Stacker,
    ];

    pub fn title(self) -> &'static str {
        match self {
            MiniKind::StopClock => "Stop the clock",
            MiniKind::QuickDraw => "Quick draw",
            MiniKind::Mash => "Mash",
            MiniKind::HighCard => "High card",
            MiniKind::CashOut => "Cash out",
            MiniKind::Stacker => "Stacker",
        }
    }

    fn window_ms(self, param: u32) -> f64 {
        match self {
            MiniKind::StopClock => 12_000.0,
            MiniKind::QuickDraw => f64::from(param) + 3_000.0,
            MiniKind::Mash => 5_500.0,
            MiniKind::HighCard => 8_000.0,
            MiniKind::CashOut => f64::from(CASH_OUT_MAX_CRASH_MS) + 1_500.0,
            MiniKind::Stacker => 15_000.0,
        }
    }

    fn higher_is_better(self) -> bool {
        matches!(
            self,
            MiniKind::Mash | MiniKind::HighCard | MiniKind::CashOut | MiniKind::Stacker
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MiniMode {
    Duel {
        challenger: PlayerId,
        target: PlayerId,
    },
    Party {
        card_player: PlayerId,
    },
}

impl MiniMode {
    pub fn card_player(self) -> PlayerId {
        match self {
            MiniMode::Duel { challenger, .. } => challenger,
            MiniMode::Party { card_player } => card_player,
        }
    }

    pub fn penalty(self) -> usize {
        match self {
            MiniMode::Duel { .. } => 3,
            MiniMode::Party { .. } => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Standing {
    pub player: PlayerId,
    pub value: Option<u32>,
    pub loser: bool,
    pub shielded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bet {
    pub player: PlayerId,
    pub on: PlayerId,
    pub amount: u32,
    pub payout: Option<i32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MiniGame {
    pub id: u32,
    pub kind: MiniKind,
    pub mode: MiniMode,
    pub participants: Vec<PlayerId>,
    pub param: u32,
    pub dealt: Vec<(PlayerId, u32)>,
    pub ready: Vec<PlayerId>,
    pub play_starts: Option<f64>,
    pub deadline: f64,
    pub submitted: Vec<(PlayerId, Option<u32>)>,
    pub standings: Option<Vec<Standing>>,
    pub turn_from: PlayerId,
    pub turn_at: Option<PlayerId>,
    pub shielded: Vec<PlayerId>,
    pub doubled_by: Option<PlayerId>,
    pub bets: Vec<Bet>,
}

impl MiniGame {
    pub fn new(
        id: u32,
        kind: MiniKind,
        mode: MiniMode,
        participants: Vec<PlayerId>,
        rng: &mut Rng,
        now: f64,
    ) -> Self {
        let param = match kind {
            MiniKind::QuickDraw => rng.range(QUICK_DRAW_MIN_DELAY_MS, QUICK_DRAW_MAX_DELAY_MS),
            MiniKind::CashOut => {
                crash_point(rng.range(CASH_OUT_MIN_CRASH_MS, CASH_OUT_MAX_CRASH_MS))
            }
            _ => 0,
        };
        let dealt = match kind {
            MiniKind::HighCard => participants
                .iter()
                .map(|&p| (p, rng.range(HIGH_CARD_LOWEST, HIGH_CARD_HIGHEST + 1)))
                .collect(),
            _ => Vec::new(),
        };
        Self {
            id,
            kind,
            mode,
            participants,
            param,
            dealt,
            ready: Vec::new(),
            play_starts: None,
            deadline: now + READY_MS,
            submitted: Vec::new(),
            standings: None,
            turn_from: mode.card_player(),
            turn_at: None,
            shielded: Vec::new(),
            doubled_by: None,
            bets: Vec::new(),
        }
    }

    pub fn is_duel(&self) -> bool {
        matches!(self.mode, MiniMode::Duel { .. })
    }

    pub fn penalty(&self) -> usize {
        self.mode.penalty() * if self.doubled_by.is_some() { 2 } else { 1 }
    }

    pub fn started(&self) -> bool {
        self.play_starts.is_some()
    }

    pub fn begin(&mut self, now: f64) {
        let play_starts = now + COUNTDOWN_MS;
        self.play_starts = Some(play_starts);
        self.deadline = play_starts + self.kind.window_ms(self.param) + GRACE_MS;
    }

    pub fn has_submitted(&self, id: PlayerId) -> bool {
        self.submitted.iter().any(|(p, _)| *p == id)
    }

    pub fn secret_for(&self, id: PlayerId) -> Option<u32> {
        if !self.participants.contains(&id) {
            return None;
        }
        match self.kind {
            MiniKind::QuickDraw | MiniKind::CashOut => Some(self.param),
            MiniKind::HighCard => self.dealt_to(id),
            _ => None,
        }
    }

    pub fn accepts(&self, value: Option<u32>) -> bool {
        match (self.kind, value) {
            (_, None) | (MiniKind::HighCard, _) => true,
            (MiniKind::StopClock, Some(v)) => (1..=60_000).contains(&v),
            (MiniKind::QuickDraw, Some(v)) => (50..=10_000).contains(&v),
            (MiniKind::Mash, Some(v)) => v <= 300,
            (MiniKind::CashOut, Some(v)) => (100..=100_000).contains(&v),
            (MiniKind::Stacker, Some(v)) => v <= 1_000,
        }
    }

    pub fn normalize(&self, value: Option<u32>) -> Option<u32> {
        match (self.kind, value) {
            (MiniKind::CashOut, Some(v)) if v > self.param => None,
            _ => value,
        }
    }

    fn dealt_to(&self, id: PlayerId) -> Option<u32> {
        self.dealt
            .iter()
            .find(|(p, _)| *p == id)
            .map(|(_, rank)| *rank)
    }

    fn value_of(&self, id: PlayerId) -> Option<u32> {
        match self.kind {
            MiniKind::HighCard => self.dealt_to(id),
            _ => self
                .submitted
                .iter()
                .find(|(p, _)| *p == id)
                .and_then(|(_, v)| *v),
        }
    }

    fn score(&self, value: Option<u32>) -> u64 {
        match value {
            None => u64::MAX,
            Some(v) if self.kind == MiniKind::StopClock => {
                u64::from(v.abs_diff(STOP_CLOCK_TARGET_MS))
            }
            Some(v) if self.kind.higher_is_better() => u64::from(u32::MAX - v),
            Some(v) => u64::from(v),
        }
    }

    pub fn rank(&self, rng: &mut Rng) -> Vec<Standing> {
        let mut rows: Vec<(PlayerId, Option<u32>)> = self
            .participants
            .iter()
            .map(|&p| (p, self.value_of(p)))
            .collect();
        rng.shuffle(&mut rows);
        rows.sort_by_key(|(_, v)| self.score(*v));
        let losers = match self.mode {
            MiniMode::Duel { .. } => 1,
            MiniMode::Party { .. } if rows.len() >= 5 => 2,
            MiniMode::Party { .. } => 1,
        };
        let cutoff = rows.len().saturating_sub(losers);
        rows.into_iter()
            .enumerate()
            .map(|(i, (player, value))| Standing {
                player,
                value,
                loser: i >= cutoff,
                shielded: self.shielded.contains(&player),
            })
            .collect()
    }
}

fn crash_point(crash_ms: u32) -> u32 {
    (100.0 * 2f64.powf(f64::from(crash_ms) / CASH_OUT_DOUBLING_MS)).round() as u32
}
