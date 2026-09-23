use serde::{Deserialize, Serialize};

use super::PlayerId;
use crate::rng::Rng;

pub const INTRO_MS: f64 = 3_000.0;
pub const RESULTS_MS: f64 = 5_000.0;
pub const STOP_CLOCK_TARGET_MS: u32 = 5_000;
const STOP_CLOCK_WINDOW_MS: f64 = 12_000.0;
const QUICK_DRAW_MIN_DELAY_MS: u32 = 1_500;
const QUICK_DRAW_MAX_DELAY_MS: u32 = 4_500;
const QUICK_DRAW_WINDOW_MS: f64 = 3_000.0;
const GRACE_MS: f64 = 2_500.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MiniKind {
    StopClock,
    QuickDraw,
}

impl MiniKind {
    pub const ALL: [MiniKind; 2] = [MiniKind::StopClock, MiniKind::QuickDraw];

    pub fn title(self) -> &'static str {
        match self {
            MiniKind::StopClock => "Stop the clock",
            MiniKind::QuickDraw => "Quick draw",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MiniMode {
    Duel { challenger: PlayerId, target: PlayerId },
    Party { card_player: PlayerId },
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MiniGame {
    pub id: u32,
    pub kind: MiniKind,
    pub mode: MiniMode,
    pub participants: Vec<PlayerId>,
    pub delay_ms: u32,
    pub play_starts: f64,
    pub deadline: f64,
    pub submitted: Vec<(PlayerId, Option<u32>)>,
    pub standings: Option<Vec<Standing>>,
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
        let delay_ms = match kind {
            MiniKind::QuickDraw => rng.range(QUICK_DRAW_MIN_DELAY_MS, QUICK_DRAW_MAX_DELAY_MS),
            MiniKind::StopClock => 0,
        };
        let window = match kind {
            MiniKind::StopClock => STOP_CLOCK_WINDOW_MS,
            MiniKind::QuickDraw => f64::from(delay_ms) + QUICK_DRAW_WINDOW_MS,
        };
        let play_starts = now + INTRO_MS;
        Self {
            id,
            kind,
            mode,
            participants,
            delay_ms,
            play_starts,
            deadline: play_starts + window + GRACE_MS,
            submitted: Vec::new(),
            standings: None,
        }
    }

    pub fn has_submitted(&self, id: PlayerId) -> bool {
        self.submitted.iter().any(|(p, _)| *p == id)
    }

    pub fn accepts(&self, value: Option<u32>) -> bool {
        match (self.kind, value) {
            (_, None) => true,
            (MiniKind::StopClock, Some(v)) => (1..=60_000).contains(&v),
            (MiniKind::QuickDraw, Some(v)) => (50..=10_000).contains(&v),
        }
    }

    fn score(&self, value: Option<u32>) -> u32 {
        match (self.kind, value) {
            (_, None) => u32::MAX,
            (MiniKind::StopClock, Some(v)) => v.abs_diff(STOP_CLOCK_TARGET_MS),
            (MiniKind::QuickDraw, Some(v)) => v,
        }
    }

    pub fn rank(&self, rng: &mut Rng) -> Vec<Standing> {
        let mut rows: Vec<(PlayerId, Option<u32>)> = self
            .participants
            .iter()
            .map(|&p| (p, self.submitted.iter().find(|(s, _)| *s == p).and_then(|(_, v)| *v)))
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
            .map(|(i, (player, value))| Standing { player, value, loser: i >= cutoff })
            .collect()
    }
}
