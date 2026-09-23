use serde::{Deserialize, Serialize};

use super::{
    Game, GameError, PlayerId, CALLOUT_MS, CALLOUT_PENALTY, EMOJI_COUNT, MAX_REACTIONS,
    REACT_COOLDOWN_MS,
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Callout {
    pub player: PlayerId,
    pub until: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    pub seq: u32,
    pub player: PlayerId,
    pub emoji: u8,
}

impl Game {
    pub(super) fn react(&mut self, id: PlayerId, emoji: u8, now: f64) -> Result<(), GameError> {
        if emoji >= EMOJI_COUNT {
            return Err(GameError::InvalidReaction);
        }
        let player = self.player_mut(id).expect("player exists");
        if now - player.last_react < REACT_COOLDOWN_MS {
            return Ok(());
        }
        player.last_react = now;
        self.reaction_seq += 1;
        self.reactions.push(Reaction {
            seq: self.reaction_seq,
            player: id,
            emoji,
        });
        if self.reactions.len() > MAX_REACTIONS {
            self.reactions.remove(0);
        }
        Ok(())
    }

    pub(super) fn expose(&mut self, id: PlayerId, now: f64) {
        self.callout = Some(Callout {
            player: id,
            until: now + CALLOUT_MS,
        });
    }

    pub(super) fn live_callout(&self, now: f64) -> Option<Callout> {
        self.callout.filter(|c| now < c.until)
    }

    pub(super) fn call(&mut self, id: PlayerId, now: f64) -> Result<(), GameError> {
        match self.live_callout(now) {
            Some(c) if c.player == id => {
                self.callout = None;
                let name = self.name(id);
                self.log(format!("{name} yelled DEALBREAKER!"));
                Ok(())
            }
            _ => Err(GameError::TooLate),
        }
    }

    pub(super) fn catch(&mut self, id: PlayerId, now: f64) -> Result<(), GameError> {
        match self.live_callout(now) {
            Some(c) if c.player != id => {
                self.callout = None;
                self.draw(c.player, CALLOUT_PENALTY);
                if let Some(p) = self.player_mut(c.player) {
                    p.stats.caught += 1;
                }
                if let Some(p) = self.player_mut(id) {
                    p.stats.catches += 1;
                }
                let (catcher, caught) = (self.name(id), self.name(c.player));
                self.log(format!("{catcher} caught {caught}! +{CALLOUT_PENALTY}"));
                Ok(())
            }
            Some(_) => Err(GameError::WrongPhase),
            None => Err(GameError::TooLate),
        }
    }
}
