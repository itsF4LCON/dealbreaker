use serde::{Deserialize, Serialize};

use super::{Game, Player, PlayerId, STARTING_COINS};

const MAX_AWARDS: usize = 6;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    pub cards_drawn: u32,
    pub duels_won: u32,
    pub fails: u32,
    pub best_stop: Option<u32>,
    pub fastest_draw: Option<u32>,
    pub lowest_cash_out: Option<u32>,
    pub caught: u32,
    pub catches: u32,
    pub tag_outs: u32,
    pub reckless: u32,
}

impl Stats {
    pub fn keep_lowest(slot: &mut Option<u32>, value: u32) {
        *slot = Some(slot.map_or(value, |old| old.min(value)));
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Award {
    pub key: &'static str,
    pub title: &'static str,
    pub player: PlayerId,
    pub detail: String,
}

fn plural(n: u32, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn most<'a>(players: &'a [Player], metric: impl Fn(&Player) -> u32) -> Option<(&'a Player, u32)> {
    players
        .iter()
        .map(|p| (p, metric(p)))
        .filter(|(_, n)| *n > 0)
        .fold(None, |best, (p, n)| match best {
            Some((_, b)) if b >= n => best,
            _ => Some((p, n)),
        })
}

fn least<'a>(
    players: &'a [Player],
    metric: impl Fn(&Player) -> Option<u32>,
) -> Option<(&'a Player, u32)> {
    players
        .iter()
        .filter_map(|p| metric(p).map(|n| (p, n)))
        .fold(None, |best, (p, n)| match best {
            Some((_, b)) if b <= n => best,
            _ => Some((p, n)),
        })
}

impl Game {
    pub fn awards(&self) -> Vec<Award> {
        let players = &self.players;
        let award = |key, title, found: Option<(&Player, u32)>, detail: &dyn Fn(u32) -> String| {
            found.map(|(p, n)| Award {
                key,
                title,
                player: p.id,
                detail: detail(n),
            })
        };
        let candidates = [
            award(
                "biggest_loser",
                "Biggest loser",
                most(players, |p| p.stats.cards_drawn),
                &|n| format!("drew {}", plural(n, "card", "cards")),
            ),
            award(
                "duelist",
                "Duelist",
                most(players, |p| p.stats.duels_won),
                &|n| format!("won {}", plural(n, "duel", "duels")),
            ),
            award("clown", "Clown", most(players, |p| p.stats.fails), &|n| {
                format!("flopped {}", plural(n, "mini-game", "mini-games"))
            }),
            award(
                "high_roller",
                "High roller",
                most(players, |p| p.coins.saturating_sub(STARTING_COINS)),
                &|n| format!("won {} betting", plural(n, "coin", "coins")),
            ),
            award(
                "butterfingers",
                "Butterfingers",
                most(players, |p| p.stats.caught),
                &|n| format!("got caught {}", plural(n, "time", "times")),
            ),
            award(
                "coward",
                "Coward",
                least(players, |p| p.stats.lowest_cash_out),
                &|n| format!("cashed out at x{:.2}", f64::from(n) / 100.0),
            ),
            award(
                "sniper",
                "Sniper",
                least(players, |p| p.stats.best_stop),
                &|n| match n {
                    0 => "stopped the clock at exactly 5.00".to_string(),
                    _ => format!("stopped the clock {:.2} s from 5.00", f64::from(n) / 1000.0),
                },
            ),
            award(
                "speed_demon",
                "Speed demon",
                least(players, |p| p.stats.fastest_draw),
                &|n| format!("{n} ms reaction"),
            ),
            award(
                "snitch",
                "Snitch",
                most(players, |p| p.stats.catches),
                &|n| format!("caught {}", plural(n, "player", "players")),
            ),
            award(
                "escape_artist",
                "Escape artist",
                most(players, |p| p.stats.tag_outs),
                &|n| format!("tagged out {}", plural(n, "time", "times")),
            ),
            award(
                "reckless",
                "Reckless",
                most(players, |p| p.stats.reckless),
                &|_| "doubled down and lost".to_string(),
            ),
            award(
                "broke",
                "Broke",
                most(players, |p| STARTING_COINS.saturating_sub(p.coins)),
                &|n| format!("lost {} betting", plural(n, "coin", "coins")),
            ),
        ];
        candidates.into_iter().flatten().take(MAX_AWARDS).collect()
    }
}
