use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}

pub const SUITS: [Suit; 4] = [Suit::Hearts, Suit::Diamonds, Suit::Clubs, Suit::Spades];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Face {
    Normal { suit: Suit, rank: u8 },
    Duel,
    Party,
    Wheel,
    TagOut,
    Shield,
    DoubleDown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub id: u16,
    pub face: Face,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Match {
    pub suit: Suit,
    pub rank: u8,
}

pub const DUELS_PER_DECK: usize = 3;
pub const PARTIES_PER_DECK: usize = 3;
pub const WHEELS_PER_DECK: usize = 2;
pub const TAG_OUTS_PER_DECK: usize = 2;
pub const SHIELDS_PER_DECK: usize = 2;
pub const DOUBLE_DOWNS_PER_DECK: usize = 1;
pub const CARDS_PER_DECK: usize = 52
    + DUELS_PER_DECK
    + PARTIES_PER_DECK
    + WHEELS_PER_DECK
    + TAG_OUTS_PER_DECK
    + SHIELDS_PER_DECK
    + DOUBLE_DOWNS_PER_DECK;

impl Card {
    pub fn is_special(&self) -> bool {
        !matches!(self.face, Face::Normal { .. })
    }

    pub fn is_counter(&self) -> bool {
        matches!(self.face, Face::TagOut | Face::Shield | Face::DoubleDown)
    }

    pub fn fits(&self, to_match: Option<Match>) -> bool {
        match (self.face, to_match) {
            (Face::Normal { suit, rank }, Some(m)) => suit == m.suit || rank == m.rank,
            (Face::Normal { .. }, None) => true,
            _ => true,
        }
    }

    pub fn title(&self) -> &'static str {
        match self.face {
            Face::Normal { .. } => "normal",
            Face::Duel => "Duel",
            Face::Party => "Party",
            Face::Wheel => "Wheel",
            Face::TagOut => "Tag out",
            Face::Shield => "Shield",
            Face::DoubleDown => "Double down",
        }
    }

    pub fn as_match(&self) -> Option<Match> {
        match self.face {
            Face::Normal { suit, rank } => Some(Match { suit, rank }),
            _ => None,
        }
    }
}

pub fn build_deck(decks: usize) -> Vec<Card> {
    let mut faces = Vec::with_capacity(decks * CARDS_PER_DECK);
    for _ in 0..decks {
        for suit in SUITS {
            for rank in 1..=13 {
                faces.push(Face::Normal { suit, rank });
            }
        }
        faces.extend(std::iter::repeat(Face::Duel).take(DUELS_PER_DECK));
        faces.extend(std::iter::repeat(Face::Party).take(PARTIES_PER_DECK));
        faces.extend(std::iter::repeat(Face::Wheel).take(WHEELS_PER_DECK));
        faces.extend(std::iter::repeat(Face::TagOut).take(TAG_OUTS_PER_DECK));
        faces.extend(std::iter::repeat(Face::Shield).take(SHIELDS_PER_DECK));
        faces.extend(std::iter::repeat(Face::DoubleDown).take(DOUBLE_DOWNS_PER_DECK));
    }
    faces
        .into_iter()
        .enumerate()
        .map(|(i, face)| Card { id: i as u16, face })
        .collect()
}
