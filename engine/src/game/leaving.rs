use super::{Game, GameError, MiniGame, Phase, PlayerId};

impl Game {
    pub(super) fn leave(&mut self, id: PlayerId, now: f64) -> Result<(), GameError> {
        let name = self.name(id);
        if self.in_lobby() {
            self.players.retain(|p| p.id != id);
            self.reassign_host(id);
            self.log(format!("{name} left"));
            return Ok(());
        }

        let next = self.step(id, 1);
        let hand = std::mem::take(self.hand_mut(id));
        self.draw_pile.extend(hand);
        self.rng.shuffle(&mut self.draw_pile);
        if self.callout.is_some_and(|c| c.player == id) {
            self.callout = None;
        }
        let phase = std::mem::replace(&mut self.phase, Phase::Lobby);
        self.players.retain(|p| p.id != id);
        self.reassign_host(id);
        self.log(format!("{name} left the game"));

        if !matches!(phase, Phase::Over { .. }) && self.players.len() == 1 {
            let winner = self.players[0].id;
            let winner_name = self.name(winner);
            self.log(format!("{winner_name} wins!"));
            self.callout = None;
            self.phase = Phase::Over { winner };
            return Ok(());
        }

        match phase {
            Phase::Turn { player, .. }
            | Phase::ChooseTarget { player, .. }
            | Phase::ChooseDiscard { player, .. }
            | Phase::Wheel { player, .. }
                if player == id =>
            {
                self.begin_turn(next, now)
            }
            Phase::MiniGame(m) => self.minigame_without(m, id, next, now),
            other => self.phase = other,
        }
        Ok(())
    }

    fn reassign_host(&mut self, gone: PlayerId) {
        if self.host == Some(gone) {
            self.host = self
                .players
                .iter()
                .find(|p| p.connected)
                .or(self.players.first())
                .map(|p| p.id);
        }
    }

    fn minigame_without(&mut self, mut m: MiniGame, id: PlayerId, next: PlayerId, now: f64) {
        if m.turn_from == id || m.turn_at == Some(id) {
            m.turn_at = Some(next);
        }
        let was_playing = m.participants.contains(&id);
        m.participants.retain(|&p| p != id);
        m.ready.retain(|&p| p != id);
        m.shielded.retain(|&p| p != id);
        m.submitted.retain(|(p, _)| *p != id);
        m.dealt.retain(|(p, _)| *p != id);
        m.bets.retain(|b| b.player != id);

        let broken =
            m.standings.is_none() && was_playing && (m.is_duel() || m.participants.len() < 2);
        if broken {
            for bet in m.bets.drain(..) {
                if let Some(p) = self.player_mut(bet.player) {
                    p.coins += bet.amount;
                }
            }
            self.log(
                if m.is_duel() {
                    "The duel is off"
                } else {
                    "The party game is off"
                }
                .into(),
            );
            self.resume_after_minigame(&m, now);
            return;
        }
        self.phase = Phase::MiniGame(m);
        self.begin_minigame_if_ready(now);
        self.finish_minigame_if_complete(now);
    }
}
