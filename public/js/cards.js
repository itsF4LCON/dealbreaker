const SUITS = {
  hearts: { symbol: "♥", red: true, order: 0 },
  diamonds: { symbol: "♦", red: true, order: 1 },
  clubs: { symbol: "♣", red: false, order: 2 },
  spades: { symbol: "♠", red: false, order: 3 },
};

const RANKS = { 1: "A", 11: "J", 12: "Q", 13: "K" };

const ICONS = {
  duel: '<path d="M5 5l14 14M19 5L5 19M3 17l4 4M17 21l4-4"/>',
  party: '<path d="M12 2v5M12 17v5M2 12h5M17 12h5M5 5l3.5 3.5M15.5 15.5L19 19M19 5l-3.5 3.5M8.5 15.5L5 19"/>',
  wheel: '<circle cx="12" cy="12" r="9"/><path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6L5.6 18.4"/>',
};

export const SPECIALS = {
  duel: { label: "DUEL", hint: "1v1, loser +3", about: "Pick a player for a 1v1 mini-game. The loser draws 3, and that can be you." },
  party: { label: "PARTY", hint: "all play, last +2", about: "Everyone plays a mini-game. Last place draws 2." },
  wheel: { label: "WHEEL", hint: "spin it", about: "Spin the wheel. Anything can happen, good or bad." },
};

export function rankLabel(rank) {
  return RANKS[rank] || String(rank);
}

export function suitSymbol(suit) {
  return SUITS[suit].symbol;
}

export function isRed(suit) {
  return SUITS[suit].red;
}

export function cardName(face) {
  if (face.kind === "normal") return `${rankLabel(face.rank)}${suitSymbol(face.suit)}`;
  return SPECIALS[face.kind].label;
}

export function cardElement(card, tag = "button") {
  const el = document.createElement(tag);
  if (tag === "button") el.type = "button";
  el.className = "card";
  el.dataset.id = card.id;
  const face = card.face;
  if (face.kind === "normal") {
    const rank = rankLabel(face.rank);
    const symbol = suitSymbol(face.suit);
    if (isRed(face.suit)) el.classList.add("red");
    el.setAttribute("aria-label", `${rank} of ${face.suit}`);
    el.innerHTML =
      `<span class="corner"><span>${rank}</span><span>${symbol}</span></span>` +
      `<span class="pip" aria-hidden="true">${symbol}</span>` +
      `<span class="corner bottom"><span>${rank}</span><span>${symbol}</span></span>`;
  } else {
    const info = SPECIALS[face.kind];
    el.classList.add("special");
    el.setAttribute("aria-label", `${info.label} card`);
    el.innerHTML =
      `<span class="label">${info.label}</span>` +
      `<svg viewBox="0 0 24 24" aria-hidden="true">${ICONS[face.kind]}</svg>` +
      `<span class="hint">${info.hint}</span>`;
  }
  return el;
}

export function sortHand(hand) {
  const key = (c) =>
    c.face.kind === "normal"
      ? SUITS[c.face.suit].order * 100 + c.face.rank
      : 1000 + ["duel", "party", "wheel"].indexOf(c.face.kind);
  return [...hand].sort((a, b) => key(a) - key(b));
}

export function renderSpecials(container) {
  container.replaceChildren(
    ...Object.keys(SPECIALS).map((kind, i) => {
      const wrap = document.createElement("div");
      wrap.className = "special-info";
      wrap.append(cardElement({ id: -1 - i, face: { kind } }, "div"));
      const p = document.createElement("p");
      p.textContent = SPECIALS[kind].about;
      wrap.append(p);
      return wrap;
    }),
  );
}
