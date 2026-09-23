use super::*;
use crate::cards::{build_deck, Face, Suit};

const T0: f64 = 1_000_000.0;

fn started(n: usize) -> (Game, Vec<PlayerId>) {
    let mut g = Game::new(7);
    let ids: Vec<PlayerId> = (0..n)
        .map(|i| {
            g.join(&format!("token-{i:02}-0123456789abcdef"), &format!("P{i}"))
                .unwrap()
        })
        .collect();
    for &id in &ids {
        g.set_connected(id, true, T0);
    }
    g.act(ids[0], Action::Start, T0).unwrap();
    (g, ids)
}

fn normal(id: u16, suit: Suit, rank: u8) -> Card {
    Card {
        id,
        face: Face::Normal { suit, rank },
    }
}

fn card(id: u16, face: Face) -> Card {
    Card { id, face }
}

fn set_hand(g: &mut Game, id: PlayerId, cards: Vec<Card>) {
    g.player_mut(id).unwrap().hand = cards;
}

fn hand_len(g: &Game, id: PlayerId) -> usize {
    g.player(id).unwrap().hand.len()
}

fn coins(g: &Game, id: PlayerId) -> u32 {
    g.player(id).unwrap().coins
}

fn total_cards(g: &Game) -> usize {
    g.draw_pile.len() + g.discard.len() + g.players.iter().map(|p| p.hand.len()).sum::<usize>()
}

fn force_turn(g: &mut Game, id: PlayerId) {
    g.phase = Phase::Turn {
        player: id,
        drawn: None,
        deadline: T0 + TURN_MS,
    };
}

fn set_match(g: &mut Game, suit: Suit, rank: u8) {
    g.discard.push(normal(999, suit, rank));
    g.to_match = Some(Match { suit, rank });
}

fn current(g: &Game) -> PlayerId {
    match g.phase {
        Phase::Turn { player, .. } => player,
        ref other => panic!("expected a turn, got {other:?}"),
    }
}

fn minigame(g: &Game) -> &MiniGame {
    match &g.phase {
        Phase::MiniGame(m) => m,
        other => panic!("expected a mini-game, got {other:?}"),
    }
}

fn duel(g: &mut Game, a: PlayerId, b: PlayerId) {
    force_turn(g, a);
    let mut hand = g.player(a).unwrap().hand.clone();
    hand.push(card(900, Face::Duel));
    set_hand(g, a, hand);
    g.act(a, Action::Play { card: 900 }, T0).unwrap();
    if matches!(g.phase, Phase::ChooseTarget { .. }) {
        g.act(a, Action::Target { player: b }, T0).unwrap();
    }
    if let Phase::MiniGame(m) = &mut g.phase {
        m.kind = MiniKind::StopClock;
    }
}

fn give(g: &mut Game, id: PlayerId, c: Card) {
    g.player_mut(id).unwrap().hand.push(c);
}

fn play_out(g: &mut Game, winner: PlayerId, loser: PlayerId) {
    let participants = minigame(g).participants.clone();
    for &p in &participants {
        if !minigame(g).ready.contains(&p) {
            g.act(p, Action::Ready, T0).unwrap();
        }
    }
    g.act(winner, Action::Result { value: Some(5_000) }, T0 + 5_000.0)
        .unwrap();
    g.act(loser, Action::Result { value: Some(9_000) }, T0 + 5_000.0)
        .unwrap();
}

#[test]
fn every_deck_has_the_reaction_cards() {
    let deck = build_deck(1);
    let count = |face: Face| deck.iter().filter(|c| c.face == face).count();
    assert_eq!(count(Face::TagOut), 2);
    assert_eq!(count(Face::Shield), 2);
    assert_eq!(count(Face::DoubleDown), 1);
    let json = serde_json::to_string(&card(1, Face::TagOut)).unwrap();
    assert!(json.contains("\"tag_out\""));
}

#[test]
fn reaction_cards_can_be_thrown_away_on_your_turn() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(
        &mut g,
        ids[0],
        vec![card(900, Face::Shield), normal(901, Suit::Clubs, 2)],
    );

    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();

    assert_eq!(current(&g), ids[1]);
    assert_eq!(
        g.to_match,
        Some(Match {
            suit: Suit::Hearts,
            rank: 7
        })
    );
    assert_eq!(g.discard.last().unwrap().id, 900);
}

#[test]
fn tag_out_sends_someone_else_into_the_duel() {
    let (mut g, ids) = started(3);
    duel(&mut g, ids[0], ids[1]);
    give(&mut g, ids[1], card(950, Face::TagOut));
    let before = hand_len(&g, ids[1]);

    g.act(
        ids[1],
        Action::Counter {
            card: 950,
            target: Some(ids[2]),
        },
        T0,
    )
    .unwrap();

    let m = minigame(&g);
    assert_eq!(m.participants, vec![ids[0], ids[2]]);
    assert_eq!(
        m.mode,
        MiniMode::Duel {
            challenger: ids[0],
            target: ids[2]
        }
    );
    assert_eq!(hand_len(&g, ids[1]), before - 1);
    assert_eq!(g.player(ids[1]).unwrap().stats.tag_outs, 1);
}

#[test]
fn tag_out_needs_a_duel_a_free_target_and_the_ready_stage() {
    let (mut g, ids) = started(3);
    duel(&mut g, ids[0], ids[1]);
    give(&mut g, ids[1], card(950, Face::TagOut));
    assert_eq!(
        g.act(
            ids[1],
            Action::Counter {
                card: 950,
                target: Some(ids[0])
            },
            T0
        ),
        Err(GameError::InvalidTarget)
    );
    assert_eq!(
        g.act(
            ids[1],
            Action::Counter {
                card: 950,
                target: None
            },
            T0
        ),
        Err(GameError::InvalidTarget)
    );
    assert_eq!(
        g.act(
            ids[2],
            Action::Counter {
                card: 950,
                target: Some(ids[2])
            },
            T0
        ),
        Err(GameError::NoSuchCard)
    );
    g.act(ids[0], Action::Ready, T0).unwrap();
    g.act(ids[1], Action::Ready, T0).unwrap();
    assert_eq!(
        g.act(
            ids[1],
            Action::Counter {
                card: 950,
                target: Some(ids[2])
            },
            T0
        ),
        Err(GameError::CardNotUsableNow)
    );
}

#[test]
fn a_reaction_card_cannot_be_your_last_card() {
    let (mut g, ids) = started(2);
    duel(&mut g, ids[0], ids[1]);
    set_hand(&mut g, ids[1], vec![card(950, Face::Shield)]);
    assert_eq!(
        g.act(
            ids[1],
            Action::Counter {
                card: 950,
                target: None
            },
            T0
        ),
        Err(GameError::SpecialLastCard)
    );
}

#[test]
fn a_shield_blocks_the_penalty() {
    let (mut g, ids) = started(2);
    duel(&mut g, ids[0], ids[1]);
    give(&mut g, ids[1], card(950, Face::Shield));
    g.act(
        ids[1],
        Action::Counter {
            card: 950,
            target: None,
        },
        T0,
    )
    .unwrap();
    let before = hand_len(&g, ids[1]);

    play_out(&mut g, ids[0], ids[1]);

    let standings = minigame(&g).standings.clone().unwrap();
    assert!(standings[1].loser && standings[1].shielded);
    assert_eq!(hand_len(&g, ids[1]), before);
}

#[test]
fn double_down_makes_the_loser_draw_six() {
    let (mut g, ids) = started(2);
    duel(&mut g, ids[0], ids[1]);
    give(&mut g, ids[1], card(950, Face::DoubleDown));
    g.act(
        ids[1],
        Action::Counter {
            card: 950,
            target: None,
        },
        T0,
    )
    .unwrap();
    assert_eq!(
        g.act(
            ids[1],
            Action::Counter {
                card: 951,
                target: None
            },
            T0
        ),
        Err(GameError::NoSuchCard)
    );
    let before = hand_len(&g, ids[1]);

    play_out(&mut g, ids[0], ids[1]);

    assert_eq!(hand_len(&g, ids[1]), before + 6);
    assert_eq!(g.player(ids[1]).unwrap().stats.reckless, 1);
}

#[test]
fn bets_on_the_winner_pay_double() {
    let (mut g, ids) = started(4);
    duel(&mut g, ids[0], ids[1]);
    g.act(
        ids[2],
        Action::Bet {
            on: ids[0],
            amount: 3,
        },
        T0,
    )
    .unwrap();
    g.act(
        ids[3],
        Action::Bet {
            on: ids[1],
            amount: 2,
        },
        T0,
    )
    .unwrap();
    assert_eq!(coins(&g, ids[2]), STARTING_COINS - 3);

    play_out(&mut g, ids[0], ids[1]);

    assert_eq!(coins(&g, ids[2]), STARTING_COINS + 3);
    assert_eq!(coins(&g, ids[3]), STARTING_COINS - 2);
    let bets = &minigame(&g).bets;
    assert_eq!(bets[0].payout, Some(3));
    assert_eq!(bets[1].payout, Some(-2));
}

#[test]
fn bets_must_follow_the_rules() {
    let (mut g, ids) = started(3);
    duel(&mut g, ids[0], ids[1]);
    assert_eq!(
        g.act(
            ids[0],
            Action::Bet {
                on: ids[1],
                amount: 1
            },
            T0
        ),
        Err(GameError::InvalidBet)
    );
    assert_eq!(
        g.act(
            ids[2],
            Action::Bet {
                on: ids[2],
                amount: 1
            },
            T0
        ),
        Err(GameError::InvalidBet)
    );
    assert_eq!(
        g.act(
            ids[2],
            Action::Bet {
                on: ids[0],
                amount: 4
            },
            T0
        ),
        Err(GameError::InvalidBet)
    );
    assert_eq!(
        g.act(
            ids[2],
            Action::Bet {
                on: ids[0],
                amount: 0
            },
            T0
        ),
        Err(GameError::InvalidBet)
    );
    g.act(
        ids[2],
        Action::Bet {
            on: ids[0],
            amount: 1,
        },
        T0,
    )
    .unwrap();
    assert_eq!(
        g.act(
            ids[2],
            Action::Bet {
                on: ids[0],
                amount: 1
            },
            T0
        ),
        Err(GameError::InvalidBet)
    );
}

#[test]
fn nobody_can_bet_on_a_party_game() {
    let (mut g, ids) = started(3);
    force_turn(&mut g, ids[0]);
    give(&mut g, ids[0], card(900, Face::Party));
    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();
    assert_eq!(
        g.act(
            ids[1],
            Action::Bet {
                on: ids[0],
                amount: 1
            },
            T0
        ),
        Err(GameError::InvalidBet)
    );
}

#[test]
fn tagging_out_refunds_bets_on_you_and_by_your_substitute() {
    let (mut g, ids) = started(4);
    duel(&mut g, ids[0], ids[1]);
    g.act(
        ids[2],
        Action::Bet {
            on: ids[0],
            amount: 2,
        },
        T0,
    )
    .unwrap();
    g.act(
        ids[3],
        Action::Bet {
            on: ids[1],
            amount: 3,
        },
        T0,
    )
    .unwrap();
    give(&mut g, ids[1], card(950, Face::TagOut));

    g.act(
        ids[1],
        Action::Counter {
            card: 950,
            target: Some(ids[2]),
        },
        T0,
    )
    .unwrap();

    assert_eq!(coins(&g, ids[2]), STARTING_COINS);
    assert_eq!(coins(&g, ids[3]), STARTING_COINS);
    assert!(minigame(&g).bets.is_empty());
}

#[test]
fn playing_down_to_one_card_opens_a_callout() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(
        &mut g,
        ids[0],
        vec![normal(900, Suit::Hearts, 2), normal(901, Suit::Clubs, 3)],
    );

    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();

    let view = g.view(ids[1], T0 + 1_000.0).unwrap();
    let callout = view.callout.unwrap();
    assert_eq!(callout.player, ids[0]);
    assert_eq!(callout.remaining, CALLOUT_MS - 1_000.0);
}

#[test]
fn calling_dealbreaker_makes_you_safe() {
    let (mut g, ids) = started(2);
    g.expose(ids[0], T0);
    assert_eq!(g.act(ids[1], Action::Call, T0), Err(GameError::TooLate));
    g.act(ids[0], Action::Call, T0 + 500.0).unwrap();
    assert_eq!(
        g.act(ids[1], Action::Catch, T0 + 600.0),
        Err(GameError::TooLate)
    );
    assert!(g.view(ids[1], T0 + 600.0).unwrap().callout.is_none());
}

#[test]
fn getting_caught_draws_two() {
    let (mut g, ids) = started(3);
    g.expose(ids[0], T0);
    let before = hand_len(&g, ids[0]);
    assert_eq!(g.act(ids[0], Action::Catch, T0), Err(GameError::WrongPhase));

    g.act(ids[2], Action::Catch, T0 + 300.0).unwrap();

    assert_eq!(hand_len(&g, ids[0]), before + CALLOUT_PENALTY);
    assert_eq!(g.player(ids[0]).unwrap().stats.caught, 1);
    assert_eq!(g.player(ids[2]).unwrap().stats.catches, 1);
    assert_eq!(
        g.act(ids[1], Action::Catch, T0 + 400.0),
        Err(GameError::TooLate)
    );
}

#[test]
fn a_callout_runs_out() {
    let (mut g, ids) = started(2);
    g.expose(ids[0], T0);
    assert_eq!(
        g.act(ids[1], Action::Catch, T0 + CALLOUT_MS),
        Err(GameError::TooLate)
    );
    assert!(g.view(ids[1], T0 + CALLOUT_MS).unwrap().callout.is_none());
}

#[test]
fn drawing_ends_the_callout() {
    let (mut g, ids) = started(2);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Clubs, 2)]);
    g.expose(ids[0], T0);
    g.draw(ids[0], 1);
    assert_eq!(g.act(ids[1], Action::Catch, T0), Err(GameError::TooLate));
}

#[test]
fn reactions_are_shared_with_a_cooldown() {
    let (mut g, ids) = started(2);
    g.act(ids[0], Action::React { emoji: 2 }, T0).unwrap();
    g.act(ids[0], Action::React { emoji: 3 }, T0 + 100.0)
        .unwrap();
    g.act(ids[1], Action::React { emoji: 0 }, T0 + 100.0)
        .unwrap();
    g.act(ids[0], Action::React { emoji: 4 }, T0 + REACT_COOLDOWN_MS)
        .unwrap();
    assert_eq!(
        g.act(ids[0], Action::React { emoji: EMOJI_COUNT }, T0 + 5_000.0),
        Err(GameError::InvalidReaction)
    );

    let view = g.view(ids[1], T0).unwrap();
    let got: Vec<(PlayerId, u8)> = view.reactions.iter().map(|r| (r.player, r.emoji)).collect();
    assert_eq!(got, vec![(ids[0], 2), (ids[1], 0), (ids[0], 4)]);
}

#[test]
fn leaving_on_your_turn_passes_it_on_and_returns_your_cards() {
    let (mut g, ids) = started(3);
    force_turn(&mut g, ids[0]);
    let total = total_cards(&g);

    g.act(ids[0], Action::Leave, T0).unwrap();

    assert_eq!(g.players.len(), 2);
    assert_eq!(current(&g), ids[1]);
    assert_eq!(total_cards(&g), total);
    assert_eq!(g.host, Some(ids[1]));
}

#[test]
fn the_last_player_left_wins() {
    let (mut g, ids) = started(2);
    g.act(ids[1], Action::Leave, T0).unwrap();
    assert!(matches!(g.phase, Phase::Over { winner } if winner == ids[0]));
}

#[test]
fn leaving_a_duel_calls_it_off_and_refunds_bets() {
    let (mut g, ids) = started(4);
    duel(&mut g, ids[0], ids[1]);
    g.act(
        ids[2],
        Action::Bet {
            on: ids[1],
            amount: 3,
        },
        T0,
    )
    .unwrap();

    g.act(ids[1], Action::Leave, T0).unwrap();

    assert_eq!(coins(&g, ids[2]), STARTING_COINS);
    assert_eq!(current(&g), ids[2]);
}

#[test]
fn a_party_game_carries_on_when_the_card_player_leaves() {
    let (mut g, ids) = started(3);
    force_turn(&mut g, ids[0]);
    give(&mut g, ids[0], card(900, Face::Party));
    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();
    if let Phase::MiniGame(m) = &mut g.phase {
        m.kind = MiniKind::StopClock;
    }

    g.act(ids[0], Action::Leave, T0).unwrap();
    play_out(&mut g, ids[1], ids[2]);
    let deadline = g.deadline().unwrap();
    g.tick(deadline);

    assert_eq!(current(&g), ids[1]);
}

#[test]
fn awards_go_to_the_standouts() {
    let (mut g, ids) = started(3);
    g.player_mut(ids[0]).unwrap().stats.cards_drawn = 14;
    g.player_mut(ids[1]).unwrap().stats.cards_drawn = 3;
    g.player_mut(ids[1]).unwrap().stats.duels_won = 2;
    g.player_mut(ids[2]).unwrap().stats.lowest_cash_out = Some(102);
    g.player_mut(ids[2]).unwrap().coins = STARTING_COINS + 4;
    g.player_mut(ids[0]).unwrap().coins = STARTING_COINS - 2;

    let awards = g.awards();
    let find = |key: &str| awards.iter().find(|a| a.key == key).cloned();

    assert_eq!(
        find("biggest_loser").map(|a| (a.player, a.detail)),
        Some((ids[0], "drew 14 cards".into()))
    );
    assert_eq!(
        find("duelist").map(|a| (a.player, a.detail)),
        Some((ids[1], "won 2 duels".into()))
    );
    assert_eq!(find("high_roller").map(|a| a.player), Some(ids[2]));
    assert_eq!(
        find("coward").map(|a| a.detail),
        Some("cashed out at x1.02".into())
    );
    assert_eq!(find("broke").map(|a| a.player), Some(ids[0]));
    assert!(find("clown").is_none());
    assert!(awards.len() <= 6);
}

#[test]
fn the_game_over_view_lists_awards_and_everyone_starts_with_coins() {
    let (mut g, ids) = started(2);
    assert!(g
        .view(ids[0], T0)
        .unwrap()
        .players
        .iter()
        .all(|p| p.coins == STARTING_COINS));
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Hearts, 1)]);
    g.draw(ids[1], 2);
    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();

    match g.view(ids[1], T0).unwrap().phase {
        PhaseView::Over { winner, awards } => {
            assert_eq!(winner, ids[0]);
            assert!(awards.iter().any(|a| a.key == "biggest_loser"));
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn the_new_actions_parse_from_client_json() {
    let parse = |s: &str| serde_json::from_str::<Action>(s).unwrap();
    assert_eq!(
        parse(r#"{"t":"react","emoji":3}"#),
        Action::React { emoji: 3 }
    );
    assert_eq!(parse(r#"{"t":"call"}"#), Action::Call);
    assert_eq!(parse(r#"{"t":"catch"}"#), Action::Catch);
    assert_eq!(
        parse(r#"{"t":"bet","on":2,"amount":3}"#),
        Action::Bet { on: 2, amount: 3 }
    );
    assert_eq!(
        parse(r#"{"t":"counter","card":7}"#),
        Action::Counter {
            card: 7,
            target: None
        }
    );
    assert_eq!(
        parse(r#"{"t":"counter","card":7,"target":4}"#),
        Action::Counter {
            card: 7,
            target: Some(4)
        }
    );
}
