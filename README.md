# Dealbreaker

A multiplayer card game for 2–8 friends in the browser. It plays like Crazy Eights with a normal deck of cards, but instead of "draw 2" cards there are cards that start a mini-game, like in Wii Party.

Create a room, send the link, and play. No accounts, no install.

## Rules

### Setup
- 2–8 players. One deck for 2–4 players, two decks shuffled together for 5–8.
- A deck is 52 normal cards plus 8 special cards: 3 **Duel**, 3 **Party** and 2 **Wheel**.
- Everyone gets 7 cards. The top card of the deck is turned over to start the pile. It is always a normal card.

### Your turn
- Play one normal card that matches the suit or the rank of the card to match, or play any special card.
- If you can't or don't want to play, draw 1. If that card fits, you may play it straight away. Otherwise your turn ends.
- A turn lasts 30 seconds. When time runs out you draw a card automatically. A player who is offline gets 5 seconds.
- After a special card, the next player still matches the last normal card.

### Special cards
- **Duel**: pick a player. You both play a random mini-game, and the loser draws 3. That can be you.
- **Party**: everyone plays a random mini-game. Last place draws 2. With 5 or more players, the bottom two draw 2.
- **Wheel**: you see the wheel and what each outcome does, then tap Spin (it spins by itself after 15 seconds). The 8 outcomes:
  - Everyone draws 1
  - You draw 2
  - Pick a player who draws 2
  - Swap hands with a player
  - Skip the next player
  - Reverse the direction
  - Everyone passes their hand to the next player
  - Throw away one card of your choice (not your last one)

### Winning
- The first player with no cards left wins.
- Your last card must be a normal card, so you can't win by playing a special card.
- When the deck runs out, the pile is shuffled back in, except the top card.

### Mini-games
A random mini-game is picked for every Duel and Party card, never the same one twice in a row. First everyone in it sees how to play and taps Ready; the game starts with a 3-2-1 once all online players are ready, or after 20 seconds.

- **Stop the clock**: a timer counts up and disappears after 2 seconds. Tap when you think it reads exactly 5.00 s. Closest wins.
- **Quick draw**: wait for the flash, then tap as fast as you can. Tapping before the flash counts as a false start and loses.
- **Mash**: tap as fast as you can for 5 seconds. Most taps wins.
- **High card**: flip your face-down card. The server deals the cards, so it's pure luck. Lowest card loses, aces are high.
- **Cash out**: a multiplier climbs from x1.00 until it crashes at a secret point between about x1.6 and x8, the same for everyone. Cash out as high as you dare. Crashing counts as the worst result.
- **Stacker**: a card slides back and forth. Tap to drop it on the stack; any overhang is cut off. After 5 drops the widest stack wins. Missing completely counts as falling off.

Special cards only show an icon. Hover over one (or hold it on a phone) to see what it does.

Each player's device measures their own result, so a slow connection doesn't decide who wins. The results are trusted, which is fine for a game between friends.

## How it works

```
browser ──WebSocket──▶ Worker (Rust) ──▶ Room Durable Object (one per room code)
                                              │
                                              └─ dealbreaker-engine: all game rules
```

- **`engine/`**: the game rules as a plain Rust crate with no Cloudflare code. It is a state machine: players send actions (`play`, `draw`, `target` …), and time moves the game on through deadlines (turn timer, mini-game windows, the wheel spin). Tested with `cargo test`.
- **`src/room.rs`**: the Durable Object for one room. It uses the WebSocket hibernation API, saves the game after every change, and uses an alarm for the next deadline. It sends each player a personal view, so nobody receives another player's hand.
- **`src/lib.rs`**: the Worker. It creates rooms with a 4-letter code, sends WebSocket connections to the right room, and serves the pages.
- **`public/`**: plain HTML, CSS and JavaScript, no framework and no build step.

Players are recognised by a random token kept in the browser's `localStorage`, so refreshing the page or reconnecting puts you back in your seat. Two tabs in the same browser are the same player; use a private window or a second browser to test alone. When the host starts a game, players who are offline at that moment are removed from the lobby, so a stale seat (for example from an in-app browser) is never dealt in. Rooms delete themselves 30 minutes after the last player leaves.

## Develop

```sh
cargo test -p dealbreaker-engine
wrangler dev
```

## Deploy

```sh
wrangler deploy
```

This serves the game on `dealbreaker.xivlabs.tech`.
