use super::{Bet, Face, Game, GameError, MiniMode, Phase, PlayerId, MAX_BET, TAG_OUT_EXTRA_MS};

impl Game {
    pub(super) fn bet(&mut self, id: PlayerId, on: PlayerId, amount: u32) -> Result<(), GameError> {
        let coins = self.player(id).map_or(0, |p| p.coins);
        let Phase::MiniGame(m) = &mut self.phase else {
            return Err(GameError::InvalidBet);
        };
        let allowed = m.is_duel()
            && m.standings.is_none()
            && !m.participants.contains(&id)
            && m.participants.contains(&on)
            && !m.bets.iter().any(|b| b.player == id)
            && (1..=MAX_BET).contains(&amount)
            && amount <= coins;
        if !allowed {
            return Err(GameError::InvalidBet);
        }
        m.bets.push(Bet {
            player: id,
            on,
            amount,
            payout: None,
        });
        self.player_mut(id).expect("player exists").coins -= amount;
        let (name, on_name) = (self.name(id), self.name(on));
        self.log(format!("{name} bet {amount} on {on_name}"));
        Ok(())
    }

    pub(super) fn counter(
        &mut self,
        id: PlayerId,
        card_id: u16,
        target: Option<PlayerId>,
        now: f64,
    ) -> Result<(), GameError> {
        let hand = &self.player(id).expect("player exists").hand;
        let index = hand
            .iter()
            .position(|c| c.id == card_id)
            .ok_or(GameError::NoSuchCard)?;
        let card = hand[index];
        if !card.is_counter() {
            return Err(GameError::CardNotUsableNow);
        }
        if hand.len() == 1 {
            return Err(GameError::SpecialLastCard);
        }
        let Phase::MiniGame(m) = &self.phase else {
            return Err(GameError::CardNotUsableNow);
        };
        if m.started() || !m.participants.contains(&id) {
            return Err(GameError::CardNotUsableNow);
        }
        match card.face {
            Face::TagOut => {
                if !m.is_duel() {
                    return Err(GameError::CardNotUsableNow);
                }
                let t = target.ok_or(GameError::InvalidTarget)?;
                if m.participants.contains(&t) || !self.player(t).is_some_and(|p| p.connected) {
                    return Err(GameError::InvalidTarget);
                }
            }
            Face::Shield if m.shielded.contains(&id) => return Err(GameError::CardNotUsableNow),
            Face::DoubleDown if !m.is_duel() || m.doubled_by.is_some() => {
                return Err(GameError::CardNotUsableNow)
            }
            _ => {}
        }

        let card = self.hand_mut(id).remove(index);
        self.discard.insert(0, card);
        let name = self.name(id);
        match card.face {
            Face::TagOut => self.tag_out(id, target.expect("checked above"), now),
            Face::Shield => {
                if let Phase::MiniGame(m) = &mut self.phase {
                    m.shielded.push(id);
                }
                self.log(format!("{name} raised a Shield"));
            }
            _ => {
                if let Phase::MiniGame(m) = &mut self.phase {
                    m.doubled_by = Some(id);
                }
                self.log(format!("{name} doubled down! The loser draws 6"));
            }
        }
        if self.hand(id).len() == 1 {
            self.expose(id, now);
        }
        Ok(())
    }

    fn tag_out(&mut self, id: PlayerId, sub: PlayerId, now: f64) {
        let Phase::MiniGame(m) = &mut self.phase else {
            return;
        };
        let refunds: Vec<(PlayerId, u32)> = m
            .bets
            .iter()
            .filter(|b| b.on == id || b.player == sub)
            .map(|b| (b.player, b.amount))
            .collect();
        m.bets.retain(|b| b.on != id && b.player != sub);
        for p in &mut m.participants {
            if *p == id {
                *p = sub;
            }
        }
        m.ready.retain(|&p| p != id);
        m.shielded.retain(|&p| p != id);
        if let MiniMode::Duel { challenger, target } = &mut m.mode {
            if *challenger == id {
                *challenger = sub;
            } else if *target == id {
                *target = sub;
            }
        }
        m.deadline = m.deadline.max(now + TAG_OUT_EXTRA_MS);
        for (player, amount) in refunds {
            if let Some(p) = self.player_mut(player) {
                p.coins += amount;
            }
        }
        if let Some(p) = self.player_mut(id) {
            p.stats.tag_outs += 1;
        }
        let (name, sub_name) = (self.name(id), self.name(sub));
        self.log(format!("{name} tagged out! {sub_name} fights instead"));
    }
}
