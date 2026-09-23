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
  duel: { name: "Duel", about: "Pick a player for a 1v1 mini-game. The loser draws 3, and that can be you." },
  party: { name: "Party", about: "Everyone plays a mini-game. Last place draws 2." },
  wheel: { name: "Wheel", about: "Spin the wheel. Anything can happen, good or bad." },
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
  return SPECIALS[face.kind].name;
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
    el.classList.add("special");
    el.dataset.special = face.kind;
    el.setAttribute("aria-label", `${SPECIALS[face.kind].name} card`);
    el.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true">${ICONS[face.kind]}</svg>`;
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
      const row = document.createElement("div");
      row.className = "special-row";
      const bubble = document.createElement("p");
      bubble.className = "bubble";
      bubble.innerHTML = `<strong>${SPECIALS[kind].name}</strong>${SPECIALS[kind].about}`;
      row.append(cardElement({ id: -1 - i, face: { kind } }, "div"), bubble);
      return row;
    }),
  );
}

let tip = null;

export function hideTip() {
  if (tip) tip.hidden = true;
}

function showTip(el, kind) {
  if (!tip) {
    tip = document.createElement("div");
    tip.className = "card-tip";
    tip.setAttribute("role", "tooltip");
    document.body.append(tip);
  }
  tip.innerHTML = `<strong>${SPECIALS[kind].name}</strong>${SPECIALS[kind].about}`;
  tip.hidden = false;
  const rect = el.getBoundingClientRect();
  const width = tip.offsetWidth;
  const center = rect.left + rect.width / 2;
  const left = Math.min(Math.max(center - width / 2, 8), innerWidth - width - 8);
  tip.style.left = `${left}px`;
  tip.style.top = `${Math.max(8, rect.top - tip.offsetHeight - 14)}px`;
  tip.style.setProperty("--arrow-x", `${center - left}px`);
}

export function attachTip(el, kind) {
  let timer = null;
  el.addEventListener("pointerenter", (e) => {
    if (e.pointerType === "mouse") showTip(el, kind);
  });
  el.addEventListener("pointerleave", (e) => {
    if (e.pointerType === "mouse") hideTip();
  });
  el.addEventListener("pointerdown", (e) => {
    if (e.pointerType === "mouse") return;
    timer = setTimeout(() => {
      el.dataset.held = "1";
      showTip(el, kind);
    }, 400);
  });
  const release = () => {
    clearTimeout(timer);
    if (el.dataset.held) setTimeout(hideTip, 1500);
  };
  el.addEventListener("pointerup", release);
  el.addEventListener("pointercancel", release);
  el.addEventListener("contextmenu", (e) => e.preventDefault());
}

export function consumeHold(el) {
  if (!el.dataset.held) return false;
  delete el.dataset.held;
  return true;
}
