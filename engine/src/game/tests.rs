use super::*;
use crate::cards::{Face, Suit, CARDS_PER_DECK};

const T0: f64 = 1_000_000.0;

fn token(i: usize) -> String {
    format!("token-{i:02}-0123456789abcdef")
}

fn lobby(n: usize) -> (Game, Vec<PlayerId>) {
    let mut g = Game::new(7);
    let ids = (0..n)
        .map(|i| g.join(&token(i), &format!("P{i}")).unwrap())
        .collect();
    (g, ids)
}

fn started(n: usize) -> (Game, Vec<PlayerId>) {
    let (mut g, ids) = lobby(n);
    for &id in &ids {
        g.set_connected(id, true, T0);
    }
    g.act(ids[0], Action::Start, T0).unwrap();
    (g, ids)
}

fn normal(id: u16, suit: Suit, rank: u8) -> Card {
    Card { id, face: Face::Normal { suit, rank } }
}

fn special(id: u16, face: Face) -> Card {
    Card { id, face }
}

fn set_hand(g: &mut Game, id: PlayerId, cards: Vec<Card>) {
    g.player_mut(id).unwrap().hand = cards;
}

fn hand_len(g: &Game, id: PlayerId) -> usize {
    g.player(id).unwrap().hand.len()
}

fn set_match(g: &mut Game, suit: Suit, rank: u8) {
    g.discard.push(normal(999, suit, rank));
    g.to_match = Some(Match { suit, rank });
}

fn force_turn(g: &mut Game, id: PlayerId) {
    g.phase = Phase::Turn { player: id, drawn: None, deadline: T0 + TURN_MS };
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

fn total_cards(g: &Game) -> usize {
    g.draw_pile.len() + g.discard.len() + g.players.iter().map(|p| p.hand.len()).sum::<usize>()
}

#[test]
fn first_player_is_host_and_rejoining_keeps_the_seat() {
    let (mut g, ids) = lobby(2);
    assert_eq!(g.host, Some(ids[0]));
    assert_eq!(g.join(&token(1), "someone else").unwrap(), ids[1]);
    assert_eq!(g.players.len(), 2);
}

#[test]
fn join_rejects_full_rooms_running_games_and_bad_names() {
    let (mut g, _) = lobby(MAX_PLAYERS);
    assert_eq!(g.join(&token(50), "late"), Err(GameError::RoomFull));

    let (mut g, _) = started(2);
    assert_eq!(g.join(&token(51), "late"), Err(GameError::InProgress));

    let mut g = Game::new(1);
    assert_eq!(g.join(&token(0), "   "), Err(GameError::NameInvalid));
    assert_eq!(g.join(&token(0), "a name that is far too long"), Err(GameError::NameInvalid));
    assert_eq!(g.join(&token(0), "  Ran  ").map(|id| g.player(id).unwrap().name.clone()), Ok("Ran".into()));
}

#[test]
fn only_the_host_can_start_and_needs_two_players() {
    let (mut g, ids) = lobby(1);
    assert_eq!(g.act(ids[0], Action::Start, T0), Err(GameError::NotEnoughPlayers));
    let id = g.join(&token(1), "P1").unwrap();
    assert_eq!(g.act(id, Action::Start, T0), Err(GameError::NotHost));
    assert!(g.act(ids[0], Action::Start, T0).is_ok());
}

#[test]
fn start_deals_seven_cards_and_turns_over_a_normal_card() {
    let (g, ids) = started(3);
    for &id in &ids {
        assert_eq!(hand_len(&g, id), HAND_SIZE);
    }
    assert!(!g.discard.last().unwrap().is_special());
    assert_eq!(g.to_match, g.discard.last().unwrap().as_match());
    assert_eq!(total_cards(&g), CARDS_PER_DECK);
    assert!(ids.contains(&current(&g)));
}

#[test]
fn five_or_more_players_use_two_decks() {
    let (g, _) = started(5);
    assert_eq!(total_cards(&g), 2 * CARDS_PER_DECK);
}

#[test]
fn playing_a_matching_card_moves_to_the_next_player() {
    let (mut g, ids) = started(3);
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Spades, 7), normal(901, Suit::Clubs, 2)]);

    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();

    assert_eq!(current(&g), ids[1]);
    assert_eq!(g.to_match, Some(Match { suit: Suit::Spades, rank: 7 }));
    assert_eq!(g.discard.last().unwrap().id, 900);
}

#[test]
fn cards_that_do_not_fit_or_are_out_of_turn_are_rejected() {
    let (mut g, ids) = started(3);
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Spades, 2), normal(901, Suit::Hearts, 2)]);
    set_hand(&mut g, ids[1], vec![normal(902, Suit::Hearts, 3)]);

    assert_eq!(g.act(ids[0], Action::Play { card: 900 }, T0), Err(GameError::CardDoesNotFit));
    assert_eq!(g.act(ids[1], Action::Play { card: 902 }, T0), Err(GameError::NotYourTurn));
    assert_eq!(g.act(ids[0], Action::Play { card: 902 }, T0), Err(GameError::NoSuchCard));
}

#[test]
fn a_special_card_cannot_be_the_last_card() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_hand(&mut g, ids[0], vec![special(900, Face::Duel)]);
    assert_eq!(g.act(ids[0], Action::Play { card: 900 }, T0), Err(GameError::SpecialLastCard));
}

#[test]
fn playing_the_last_normal_card_wins() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Hearts, 1)]);

    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();

    assert!(matches!(g.phase, Phase::Over { winner } if winner == ids[0]));
    assert_eq!(g.deadline(), None);
}

#[test]
fn drawing_a_playable_card_allows_only_that_card() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Hearts, 2)]);
    g.draw_pile.push(normal(901, Suit::Hearts, 9));

    g.act(ids[0], Action::Draw, T0).unwrap();

    assert!(matches!(g.phase, Phase::Turn { drawn: Some(901), .. }));
    assert_eq!(g.act(ids[0], Action::Play { card: 900 }, T0), Err(GameError::OnlyDrawnCard));
    g.act(ids[0], Action::Play { card: 901 }, T0).unwrap();
    assert_eq!(current(&g), ids[1]);
}

#[test]
fn drawing_an_unplayable_card_ends_the_turn() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    g.draw_pile.push(normal(901, Suit::Clubs, 2));

    g.act(ids[0], Action::Draw, T0).unwrap();

    assert_eq!(current(&g), ids[1]);
    assert!(g.player(ids[0]).unwrap().hand.iter().any(|c| c.id == 901));
}

#[test]
fn passing_is_only_allowed_after_drawing() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    assert_eq!(g.act(ids[0], Action::Pass, T0), Err(GameError::WrongPhase));
}

#[test]
fn running_out_of_time_draws_a_card_and_ends_the_turn() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    let before = hand_len(&g, ids[0]);

    g.tick(T0 + TURN_MS - 1.0);
    assert_eq!(current(&g), ids[0]);
    g.tick(T0 + TURN_MS);

    assert_eq!(current(&g), ids[1]);
    assert_eq!(hand_len(&g, ids[0]), before + 1);
}

#[test]
fn offline_players_get_a_short_turn() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    g.set_connected(ids[0], false, T0);
    assert_eq!(g.deadline(), Some(T0 + OFFLINE_TURN_MS));
}

#[test]
fn host_moves_to_a_connected_player_when_the_host_leaves() {
    let (mut g, ids) = started(3);
    g.set_connected(ids[0], false, T0);
    assert_eq!(g.host, Some(ids[1]));
}

#[test]
fn a_duel_between_two_players_starts_right_away_and_the_loser_draws_three() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_hand(&mut g, ids[0], vec![special(900, Face::Duel), normal(901, Suit::Clubs, 2)]);
    let target_before = hand_len(&g, ids[1]);

    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();
    let m = minigame(&g).clone();
    assert_eq!(m.mode, MiniMode::Duel { challenger: ids[0], target: ids[1] });

    let (good, bad) = match m.kind {
        MiniKind::StopClock => (Some(5_010), Some(6_500)),
        MiniKind::QuickDraw => (Some(210), Some(480)),
    };
    g.act(ids[0], Action::Result { value: good }, T0 + 4_000.0).unwrap();
    assert_eq!(g.act(ids[0], Action::Result { value: good }, T0 + 4_000.0), Err(GameError::WrongPhase));
    g.act(ids[1], Action::Result { value: bad }, T0 + 4_100.0).unwrap();

    let standings = minigame(&g).standings.clone().unwrap();
    assert_eq!(standings[0].player, ids[0]);
    assert!(standings[1].loser);
    assert_eq!(hand_len(&g, ids[1]), target_before + 3);

    g.tick(T0 + 4_100.0 + RESULTS_MS);
    assert_eq!(current(&g), ids[1]);
}

#[test]
fn a_duel_with_more_players_asks_for_a_target() {
    let (mut g, ids) = started(3);
    force_turn(&mut g, ids[0]);
    set_hand(&mut g, ids[0], vec![special(900, Face::Duel), normal(901, Suit::Clubs, 2)]);

    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();
    assert!(matches!(g.phase, Phase::ChooseTarget { purpose: TargetPurpose::Duel, .. }));
    assert_eq!(g.act(ids[0], Action::Target { player: ids[0] }, T0), Err(GameError::InvalidTarget));

    g.act(ids[0], Action::Target { player: ids[2] }, T0).unwrap();
    assert_eq!(minigame(&g).participants, vec![ids[0], ids[2]]);
}

#[test]
fn not_picking_a_target_in_time_picks_one_at_random() {
    let (mut g, ids) = started(3);
    force_turn(&mut g, ids[0]);
    set_hand(&mut g, ids[0], vec![special(900, Face::Duel), normal(901, Suit::Clubs, 2)]);
    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();

    g.tick(T0 + CHOOSE_MS);

    let m = minigame(&g);
    assert_eq!(m.participants.len(), 2);
    assert_eq!(m.participants[0], ids[0]);
}

#[test]
fn party_with_five_players_makes_the_bottom_two_draw_two() {
    let (mut g, ids) = started(5);
    force_turn(&mut g, ids[0]);
    set_hand(&mut g, ids[0], vec![special(900, Face::Party), normal(901, Suit::Clubs, 2)]);
    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();
    let kind = minigame(&g).kind;
    let before: Vec<usize> = ids.iter().map(|&id| hand_len(&g, id)).collect();

    let values: Vec<Option<u32>> = match kind {
        MiniKind::StopClock => vec![Some(5_000), Some(5_100), Some(5_200), Some(7_000), None],
        MiniKind::QuickDraw => vec![Some(200), Some(250), Some(300), Some(900), None],
    };
    for (&id, &value) in ids.iter().zip(&values) {
        g.act(id, Action::Result { value }, T0 + 5_000.0).unwrap();
    }

    let losers: Vec<PlayerId> = minigame(&g)
        .standings
        .as_ref()
        .unwrap()
        .iter()
        .filter(|s| s.loser)
        .map(|s| s.player)
        .collect();
    assert_eq!(losers, vec![ids[3], ids[4]]);
    assert_eq!(hand_len(&g, ids[3]), before[3] + 2);
    assert_eq!(hand_len(&g, ids[4]), before[4] + 2);
    assert_eq!(hand_len(&g, ids[2]), before[2]);
}

#[test]
fn a_quick_draw_false_start_loses() {
    let mut m = MiniGame::new(
        1,
        MiniKind::QuickDraw,
        MiniMode::Duel { challenger: 1, target: 2 },
        vec![1, 2],
        &mut Rng::new(3),
        T0,
    );
    m.submitted = vec![(1, None), (2, Some(900))];
    let standings = m.rank(&mut Rng::new(3));
    assert_eq!(standings[0].player, 2);
    assert!(standings[1].loser);
}

#[test]
fn stop_the_clock_ranks_by_distance_from_five_seconds() {
    let mut m = MiniGame::new(
        1,
        MiniKind::StopClock,
        MiniMode::Party { card_player: 1 },
        vec![1, 2, 3],
        &mut Rng::new(3),
        T0,
    );
    m.submitted = vec![(1, Some(5_400)), (2, Some(4_900)), (3, Some(5_200))];
    let order: Vec<PlayerId> = m.rank(&mut Rng::new(3)).iter().map(|s| s.player).collect();
    assert_eq!(order, vec![2, 3, 1]);
}

#[test]
fn a_mini_game_ends_at_its_deadline_even_if_someone_never_answers() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_hand(&mut g, ids[0], vec![special(900, Face::Party), normal(901, Suit::Clubs, 2)]);
    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();
    let value = match minigame(&g).kind {
        MiniKind::StopClock => Some(5_000),
        MiniKind::QuickDraw => Some(250),
    };
    g.act(ids[0], Action::Result { value }, T0 + 4_000.0).unwrap();
    assert!(minigame(&g).standings.is_none());

    let deadline = g.deadline().unwrap();
    g.tick(deadline);

    let standings = minigame(&g).standings.clone().unwrap();
    assert_eq!(standings[1].player, ids[1]);
    assert!(standings[1].loser);
}

#[test]
fn a_mini_game_does_not_wait_for_offline_players() {
    let (mut g, ids) = started(3);
    force_turn(&mut g, ids[0]);
    set_hand(&mut g, ids[0], vec![special(900, Face::Party), normal(901, Suit::Clubs, 2)]);
    g.set_connected(ids[2], false, T0);
    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();
    let value = match minigame(&g).kind {
        MiniKind::StopClock => Some(5_000),
        MiniKind::QuickDraw => Some(250),
    };
    g.act(ids[0], Action::Result { value }, T0 + 4_000.0).unwrap();
    g.act(ids[1], Action::Result { value }, T0 + 4_000.0).unwrap();
    assert!(minigame(&g).standings.is_some());
}

fn spin(g: &mut Game, id: PlayerId, outcome: WheelOutcome) {
    g.phase = Phase::Wheel { player: id, outcome, deadline: T0 + SPIN_MS };
    g.tick(T0 + SPIN_MS);
}

#[test]
fn playing_a_wheel_card_spins_the_wheel() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_hand(&mut g, ids[0], vec![special(900, Face::Wheel), normal(901, Suit::Clubs, 2)]);
    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();
    assert!(matches!(g.phase, Phase::Wheel { player, .. } if player == ids[0]));
    assert_eq!(g.deadline(), Some(T0 + SPIN_MS));
}

#[test]
fn wheel_everyone_draws_one_and_you_draw_two() {
    let (mut g, ids) = started(3);
    let before: Vec<usize> = ids.iter().map(|&id| hand_len(&g, id)).collect();
    spin(&mut g, ids[0], WheelOutcome::EveryoneDraws);
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(hand_len(&g, id), before[i] + 1);
    }
    spin(&mut g, ids[0], WheelOutcome::YouDraw);
    assert_eq!(hand_len(&g, ids[0]), before[0] + 3);
    assert_eq!(current(&g), ids[1]);
}

#[test]
fn wheel_reverse_and_skip_change_who_is_next() {
    let (mut g, ids) = started(4);
    spin(&mut g, ids[1], WheelOutcome::Reverse);
    assert_eq!(current(&g), ids[0]);
    spin(&mut g, ids[1], WheelOutcome::SkipNext);
    assert_eq!(current(&g), ids[3]);
}

#[test]
fn wheel_pick_a_player_who_draws_two() {
    let (mut g, ids) = started(3);
    let before = hand_len(&g, ids[2]);
    spin(&mut g, ids[0], WheelOutcome::PickDraw);
    g.act(ids[0], Action::Target { player: ids[2] }, T0).unwrap();
    assert_eq!(hand_len(&g, ids[2]), before + 2);
    assert_eq!(current(&g), ids[1]);
}

#[test]
fn wheel_swap_hands() {
    let (mut g, ids) = started(3);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Clubs, 2)]);
    set_hand(&mut g, ids[2], vec![normal(901, Suit::Clubs, 3), normal(902, Suit::Clubs, 4)]);
    spin(&mut g, ids[0], WheelOutcome::SwapHands);
    g.act(ids[0], Action::Target { player: ids[2] }, T0).unwrap();
    assert_eq!(hand_len(&g, ids[0]), 2);
    assert_eq!(g.player(ids[2]).unwrap().hand[0].id, 900);
}

#[test]
fn wheel_pass_hands_moves_every_hand_to_the_next_player() {
    let (mut g, ids) = started(3);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Clubs, 2)]);
    set_hand(&mut g, ids[1], vec![normal(901, Suit::Clubs, 3)]);
    set_hand(&mut g, ids[2], vec![normal(902, Suit::Clubs, 4)]);
    spin(&mut g, ids[0], WheelOutcome::PassHands);
    assert_eq!(g.player(ids[1]).unwrap().hand[0].id, 900);
    assert_eq!(g.player(ids[2]).unwrap().hand[0].id, 901);
    assert_eq!(g.player(ids[0]).unwrap().hand[0].id, 902);
}

#[test]
fn wheel_throw_away_removes_a_card_but_keeps_the_card_to_match() {
    let (mut g, ids) = started(2);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Clubs, 2), normal(901, Suit::Clubs, 3)]);
    spin(&mut g, ids[0], WheelOutcome::ThrowAway);

    g.act(ids[0], Action::Discard { card: 900 }, T0).unwrap();

    assert_eq!(hand_len(&g, ids[0]), 1);
    assert_eq!(g.discard.last().unwrap().id, 999);
    assert_eq!(g.to_match, Some(Match { suit: Suit::Hearts, rank: 7 }));
    assert_eq!(current(&g), ids[1]);
}

#[test]
fn wheel_throw_away_is_skipped_with_one_card_left() {
    let (mut g, ids) = started(2);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Clubs, 2)]);
    spin(&mut g, ids[0], WheelOutcome::ThrowAway);
    assert_eq!(hand_len(&g, ids[0]), 1);
    assert_eq!(current(&g), ids[1]);
}

#[test]
fn an_empty_deck_is_refilled_from_the_pile_except_the_top_card() {
    let (mut g, ids) = started(2);
    g.draw_pile.clear();
    g.discard = vec![normal(900, Suit::Clubs, 2), normal(901, Suit::Clubs, 3), normal(902, Suit::Hearts, 4)];
    let before = hand_len(&g, ids[0]);

    let drawn = g.draw(ids[0], 1);

    assert_eq!(drawn.len(), 1);
    assert_eq!(hand_len(&g, ids[0]), before + 1);
    assert_eq!(g.discard.len(), 1);
    assert_eq!(g.discard[0].id, 902);
    assert_eq!(g.draw_pile.len(), 1);
}

#[test]
fn the_view_shows_only_your_own_hand() {
    let (g, ids) = started(3);
    let view = g.view(ids[1], T0).unwrap();
    let json = serde_json::to_string(&view).unwrap();

    assert_eq!(view.hand.len(), HAND_SIZE);
    assert_eq!(view.hand, g.player(ids[1]).unwrap().hand.as_slice());
    assert!(!json.contains("token-"));
    for p in &view.players {
        assert_eq!(p.cards, HAND_SIZE);
    }
    assert!(g.view(12_345, T0).is_none());
}

#[test]
fn the_view_shows_playable_cards_only_to_the_current_player() {
    let (mut g, ids) = started(2);
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Hearts, 2), normal(901, Suit::Clubs, 3), special(902, Face::Wheel)]);

    match g.view(ids[0], T0).unwrap().phase {
        PhaseView::Turn { playable, .. } => assert_eq!(playable, vec![900, 902]),
        other => panic!("unexpected {other:?}"),
    }
    match g.view(ids[1], T0).unwrap().phase {
        PhaseView::Turn { playable, .. } => assert!(playable.is_empty()),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn play_again_returns_to_the_lobby_and_drops_offline_players() {
    let (mut g, ids) = started(3);
    force_turn(&mut g, ids[0]);
    set_match(&mut g, Suit::Hearts, 7);
    set_hand(&mut g, ids[0], vec![normal(900, Suit::Hearts, 1)]);
    g.act(ids[0], Action::Play { card: 900 }, T0).unwrap();
    g.set_connected(ids[2], false, T0);

    assert_eq!(g.act(ids[1], Action::Again, T0), Err(GameError::NotHost));
    g.act(ids[0], Action::Again, T0).unwrap();

    assert!(matches!(g.phase, Phase::Lobby));
    assert_eq!(g.players.len(), 2);
    assert!(g.players.iter().all(|p| p.hand.is_empty()));
    g.act(ids[0], Action::Start, T0).unwrap();
}

#[test]
fn leaving_the_lobby_removes_the_player() {
    let (mut g, ids) = lobby(2);
    g.act(ids[0], Action::Leave, T0).unwrap();
    assert_eq!(g.players.len(), 1);
    assert_eq!(g.host, Some(ids[1]));
}

#[test]
fn the_game_survives_a_save_and_load() {
    let (mut g, ids) = started(4);
    force_turn(&mut g, ids[0]);
    let json = serde_json::to_string(&g).unwrap();
    let mut loaded: Game = serde_json::from_str(&json).unwrap();

    assert_eq!(total_cards(&loaded), total_cards(&g));
    loaded.act(ids[0], Action::Draw, T0).unwrap();
    g.act(ids[0], Action::Draw, T0).unwrap();
    assert_eq!(hand_len(&loaded, ids[0]), hand_len(&g, ids[0]));
}

#[test]
fn actions_parse_from_client_json() {
    let action: Action = serde_json::from_str(r#"{"t":"play","card":12}"#).unwrap();
    assert_eq!(action, Action::Play { card: 12 });
    let action: Action = serde_json::from_str(r#"{"t":"result","value":null}"#).unwrap();
    assert_eq!(action, Action::Result { value: None });
}
