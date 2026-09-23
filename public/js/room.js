import { attachTip, cardElement, consumeHold, hideTip, isRed, rankLabel, renderSpecials, sortHand, suitSymbol } from "./cards.js";
import { GAMES, formatResult, runMinigame } from "./minigames.js";
import { spinWheel } from "./wheel.js";
import { load, save } from "./store.js";

const $ = (id) => document.getElementById(id);
const code = (location.pathname.split("/")[2] || "").toUpperCase();
const SCREENS = ["loading", "missing", "name", "lobby", "table"];
const TARGET_TEXT = {
  duel: { title: "Who do you want to duel?", note: "The loser draws 3." },
  wheel_draw: { title: "Who draws 2?", note: "Pick a player." },
  wheel_swap: { title: "Swap hands with who?", note: "You get their cards, they get yours." },
};

let ws = null;
let view = null;
let joined = false;
let leaving = false;
let retryMs = 1000;
let lastSeq = null;
let overlayKey = null;
let minigameAbort = null;
const timer = { key: null, endsAt: 0, total: 1 };

function token() {
  const key = `db-token-${code}`;
  let value = load(key);
  if (!value || !/^[a-z0-9]{32}$/.test(value)) {
    const bytes = crypto.getRandomValues(new Uint8Array(16));
    value = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
    save(key, value);
  }
  return value;
}

function show(screen) {
  for (const s of SCREENS) $(`screen-${s}`).hidden = s !== screen;
}

function escapeHTML(text) {
  return String(text).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

function send(message) {
  if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(message));
}

function toast(text, kind = "") {
  const el = document.createElement("div");
  el.className = `toast ${kind}`;
  el.textContent = text;
  $("toasts").append(el);
  setTimeout(() => el.remove(), 3600);
  while ($("toasts").children.length > 4) $("toasts").firstChild.remove();
}

function setConnection(state) {
  const dot = $("conn");
  dot.className = `conn ${state}`;
  dot.title = { online: "Connected", offline: "Reconnecting…", "": "Connecting…" }[state];
}

async function boot() {
  $("room-code").textContent = code;
  document.title = `Dealbreaker – ${code}`;
  let res;
  try {
    res = await fetch(`/api/rooms/${code}`);
  } catch {
    setTimeout(boot, 2000);
    return;
  }
  if (res.status === 404) return show("missing");
  const name = load("db-name");
  if (!name) return askName();
  connect();
}

function askName(message) {
  show("name");
  const input = $("name-input");
  input.value = load("db-name") || "";
  $("name-error").hidden = !message;
  $("name-error").textContent = message || "";
  input.focus();
}

$("name-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const name = $("name-input").value.trim();
  if (!name) return;
  save("db-name", name);
  leaving = false;
  if (ws && ws.readyState === WebSocket.OPEN) {
    send({ t: "join", name, token: token() });
  } else {
    connect();
  }
});

function connect() {
  const scheme = location.protocol === "https:" ? "wss" : "ws";
  ws = new WebSocket(`${scheme}://${location.host}/api/rooms/${code}/ws`);
  setConnection("");
  ws.addEventListener("open", () => {
    retryMs = 1000;
    setConnection("online");
    send({ t: "join", name: load("db-name") || $("name-input").value.trim(), token: token() });
  });
  ws.addEventListener("message", (e) => {
    let msg;
    try {
      msg = JSON.parse(e.data);
    } catch {
      return;
    }
    if (msg.t === "state") {
      joined = true;
      view = msg.view;
      render();
    } else if (msg.t === "error") {
      if (joined) toast(msg.message, "error");
      else askName(msg.message);
    } else if (msg.t === "left") {
      leaving = true;
      location.href = "/";
    }
  });
  ws.addEventListener("close", (e) => {
    setConnection("offline");
    if (e.code === 4404) return show("missing");
    if (leaving) return;
    setTimeout(connect, retryMs);
    retryMs = Math.min(retryMs * 2, 8000);
  });
}

function player(id) {
  return view.players.find((p) => p.id === id);
}

function nameOf(id) {
  if (id === view.you) return "You";
  return player(id)?.name ?? "Someone";
}

function possessive(id) {
  return id === view.you ? "Your" : `${nameOf(id)}'s`;
}

function render() {
  const phase = view.phase;
  document.body.classList.toggle("at-table", phase.kind !== "lobby");
  showEvents();
  if (phase.kind === "lobby") {
    renderLobby();
  } else {
    renderTable();
  }
  renderOverlay();
  resetTimer();
}

function showEvents() {
  const newest = view.events.length ? view.events[view.events.length - 1].seq : 0;
  if (lastSeq !== null && view.phase.kind !== "lobby") {
    for (const e of view.events) if (e.seq > lastSeq) toast(e.text);
  }
  lastSeq = newest;
}

function renderLobby() {
  show("lobby");
  $("lobby-code").textContent = code;
  $("invite").value = `${location.origin}/r/${code}`;
  $("player-count").textContent = `${view.players.length}/8`;
  $("lobby-players").innerHTML = view.players
    .map((p) => {
      const tags = [
        p.id === view.host ? '<span class="tag dark">host</span>' : "",
        p.id === view.you ? '<span class="tag">you</span>' : "",
        p.connected ? "" : '<span class="tag">offline</span>',
      ].join(" ");
      return `<li class="${p.connected ? "" : "offline"}"><span>${escapeHTML(p.name)}</span><span>${tags}</span></li>`;
    })
    .join("");
  const host = view.host === view.you;
  const online = view.players.filter((p) => p.connected).length;
  $("start").hidden = !host;
  $("start").disabled = online < 2;
  $("start").textContent = online < 2 ? "Waiting for a second player…" : "Start the game";
  $("lobby-wait").hidden = host;
  $("lobby-wait").textContent = `Waiting for ${player(view.host)?.name ?? "the host"} to start the game.`;
}

function renderTable() {
  show("table");
  const phase = view.phase;
  const active = phase.player ?? null;

  $("seats").innerHTML = view.players
    .map((p) => {
      const classes = ["seat", p.id === active ? "active" : "", p.id === view.you ? "me" : "", p.connected ? "" : "offline"];
      return `<li class="${classes.join(" ")}"><span class="name">${escapeHTML(p.name)}</span><span class="count">${p.cards}</span></li>`;
    })
    .join("");

  const myTurn = phase.kind === "turn" && phase.player === view.you;
  const drawPile = $("draw-pile");
  drawPile.disabled = !(myTurn && phase.drawn === null);
  drawPile.classList.toggle("ready", myTurn && phase.drawn === null);
  $("pile-count").textContent = `${view.pile} left`;

  const discard = $("discard");
  const top = view.top ? cardElement(view.top, "div") : "";
  if (top && top.dataset.special) attachTip(top, top.dataset.special);
  discard.replaceChildren(top);
  const m = view.to_match;
  const topSpecial = view.top && view.top.face.kind !== "normal";
  $("match").innerHTML = m && topSpecial
    ? `Match <span class="${isRed(m.suit) ? "red-text" : ""}">${suitSymbol(m.suit)}</span> or ${rankLabel(m.rank)}`
    : `direction ${view.direction === 1 ? "→" : "←"}`;

  $("status").innerHTML = statusText();
  $("pass").hidden = !(myTurn && phase.drawn !== null);
  renderHand();
}

function statusText() {
  const phase = view.phase;
  const mine = phase.player === view.you;
  switch (phase.kind) {
    case "turn":
      if (!mine) return `${escapeHTML(possessive(phase.player))} turn`;
      if (phase.drawn !== null) return "You drew a card that fits. Play it or keep it.";
      return phase.playable.length ? "Your turn" : 'Your turn <span class="muted">nothing fits, draw a card</span>';
    case "target":
      return mine ? "Pick a player" : `${escapeHTML(nameOf(phase.player))} is picking a player`;
    case "discard":
      return mine ? "Tap a card to throw away" : `${escapeHTML(nameOf(phase.player))} is throwing a card away`;
    case "minigame":
      return "Mini-game!";
    case "wheel":
      return `${escapeHTML(nameOf(phase.player))} spins the wheel`;
    case "over":
      return `${escapeHTML(nameOf(phase.winner))} ${phase.winner === view.you ? "win" : "wins"}!`;
    default:
      return "";
  }
}

function renderHand() {
  const phase = view.phase;
  const myTurn = phase.kind === "turn" && phase.player === view.you;
  const discarding = phase.kind === "discard" && phase.player === view.you;
  const playable = new Set(myTurn ? phase.playable : []);
  const hand = $("hand");
  hideTip();
  hand.replaceChildren(
    ...sortHand(view.hand).map((card) => {
      const el = cardElement(card);
      if (el.dataset.special) attachTip(el, el.dataset.special);
      if (myTurn) el.classList.add(playable.has(card.id) ? "playable" : "dim");
      if (discarding) el.classList.add("pickable");
      el.addEventListener("click", () => onCard(card, el));
      return el;
    }),
  );
}

function onCard(card, el) {
  if (consumeHold(el)) return;
  const phase = view.phase;
  if (phase.kind === "turn" && phase.player === view.you) {
    if (phase.playable.includes(card.id)) return send({ t: "play", card: card.id });
    el.classList.remove("nope");
    void el.offsetWidth;
    el.classList.add("nope");
    return;
  }
  if (phase.kind === "discard" && phase.player === view.you) {
    send({ t: "discard", card: card.id });
  }
}

$("draw-pile").addEventListener("click", () => send({ t: "draw" }));
$("pass").addEventListener("click", () => send({ t: "pass" }));
$("start").addEventListener("click", () => send({ t: "start" }));
$("leave").addEventListener("click", () => {
  leaving = true;
  send({ t: "leave" });
  setTimeout(() => (location.href = "/"), 400);
});
$("copy-invite").addEventListener("click", async (e) => {
  try {
    await navigator.clipboard.writeText($("invite").value);
    e.target.textContent = "Copied";
    setTimeout(() => (e.target.textContent = "Copy"), 1600);
  } catch {
    $("invite").select();
  }
});

function overlayKeyFor(phase) {
  switch (phase.kind) {
    case "target":
      return phase.player === view.you ? `target:${phase.purpose}` : null;
    case "minigame":
      return `mini:${phase.id}:${phase.results ? "results" : "play"}`;
    case "wheel":
      return `wheel:${phase.player}:${phase.outcome}`;
    case "over":
      return `over:${phase.winner}`;
    default:
      return null;
  }
}

function renderOverlay() {
  const overlay = $("overlay");
  const phase = view.phase;
  const key = overlayKeyFor(phase);
  if (key !== overlayKey) {
    if (minigameAbort) minigameAbort.abort();
    minigameAbort = null;
    overlayKey = key;
    overlay.className = "overlay";
    overlay.hidden = key === null;
    overlay.replaceChildren();
    if (key === null) return;
    if (phase.kind === "target") buildTarget(overlay, phase);
    if (phase.kind === "minigame" && !phase.results) buildMinigame(overlay, phase);
    if (phase.kind === "minigame" && phase.results) buildResults(overlay, phase);
    if (phase.kind === "wheel") buildWheel(overlay, phase);
    if (phase.kind === "over") buildOver(overlay);
  }
  if (phase.kind === "minigame" && !phase.results) updateWatchers(phase);
  if (phase.kind === "over") updateOver();
}

function buildTarget(overlay, phase) {
  const text = TARGET_TEXT[phase.purpose];
  const dialog = document.createElement("div");
  dialog.className = "dialog";
  dialog.innerHTML = `<h2>${text.title}</h2><p>${text.note}</p><div class="choices"></div>`;
  const choices = dialog.querySelector(".choices");
  for (const id of phase.options) {
    const p = player(id);
    const b = document.createElement("button");
    b.type = "button";
    b.innerHTML = `<span>${escapeHTML(p.name)}</span><span class="count">${p.cards} cards</span>`;
    b.addEventListener("click", () => send({ t: "target", player: id }));
    choices.append(b);
  }
  overlay.append(dialog);
}

function modeLines(phase) {
  if (phase.mode.type === "duel") {
    return { mode: "DUEL", who: `${nameOf(phase.mode.challenger)} vs ${nameOf(phase.mode.target)}` };
  }
  return { mode: "PARTY", who: "Everyone plays" };
}

function buildMinigame(overlay, phase) {
  overlay.classList.add("night");
  const { mode, who } = modeLines(phase);
  const info = GAMES[phase.game];
  const stage = document.createElement("div");
  stage.className = "night-stage";
  stage.innerHTML =
    `<p class="mg-mode">${mode}</p>` +
    `<p class="mg-who">${escapeHTML(who)}</p>` +
    `<p class="mg-title">${info.title}. ${info.howto}</p>` +
    '<div class="mg-area"></div>';
  overlay.append(stage);
  const area = stage.querySelector(".mg-area");
  const playing = phase.participants.includes(view.you) && !phase.submitted.includes(view.you);
  const abort = new AbortController();
  minigameAbort = abort;
  runMinigame({ area, game: phase.game, startsIn: phase.starts_in, param: phase.param, playing, signal: abort.signal }).then(
    (value) => {
      if (abort.signal.aborted) return;
      if (value === undefined) {
        area.innerHTML = '<ul class="mg-watch"></ul>';
        updateWatchers(view.phase);
      } else {
        send({ t: "result", value });
      }
    },
  );
}

function updateWatchers(phase) {
  const list = document.querySelector("#overlay .mg-watch");
  if (!list || phase.kind !== "minigame") return;
  list.innerHTML = phase.participants
    .map((id) => `<li class="${phase.submitted.includes(id) ? "done" : ""}">${escapeHTML(nameOf(id))}</li>`)
    .join("");
}

function buildResults(overlay, phase) {
  overlay.classList.add("night");
  const { mode } = modeLines(phase);
  const stage = document.createElement("div");
  stage.className = "night-stage";
  const rows = phase.results
    .map(
      (s, i) =>
        `<li class="${s.loser ? "loser" : ""}">` +
        `<span class="place">${i + 1}</span>` +
        `<span class="name">${escapeHTML(nameOf(s.player))}</span>` +
        `<span class="value">${formatResult(phase.game, s.value)}</span>` +
        (s.loser ? `<span class="penalty">draws ${phase.penalty}</span>` : "") +
        "</li>",
    )
    .join("");
  stage.innerHTML = `<p class="mg-title">${mode} RESULT, ${GAMES[phase.game].title}</p><ol class="results">${rows}</ol>`;
  overlay.append(stage);
}

function buildWheel(overlay, phase) {
  overlay.classList.add("night");
  const stage = document.createElement("div");
  stage.className = "night-stage";
  stage.innerHTML = `<p class="mg-title">${escapeHTML(nameOf(phase.player))} ${phase.player === view.you ? "spin" : "spins"} the wheel</p><div class="wheel-area"></div>`;
  overlay.append(stage);
  spinWheel(stage.querySelector(".wheel-area"), phase.outcome, nameOf(phase.player));
}

function buildOver(overlay) {
  overlay.classList.add("night");
  const stage = document.createElement("div");
  stage.className = "night-stage";
  stage.innerHTML =
    '<p class="mg-title">Game over</p>' +
    '<p class="winner"></p>' +
    '<div class="night-actions"><button type="button" id="again">Play again</button><a class="button ghost" href="/">Leave</a></div>' +
    '<p class="mg-note" id="again-wait"></p>';
  overlay.append(stage);
  stage.querySelector("#again").addEventListener("click", () => send({ t: "again" }));
}

function updateOver() {
  const phase = view.phase;
  const winner = document.querySelector("#overlay .winner");
  if (!winner) return;
  winner.textContent = phase.winner === view.you ? "You win!" : `${nameOf(phase.winner)} wins!`;
  const host = view.host === view.you;
  document.getElementById("again").hidden = !host;
  document.getElementById("again-wait").textContent = host ? "" : `Waiting for ${player(view.host)?.name ?? "the host"} to start a new game.`;
}

function resetTimer() {
  const phase = view.phase;
  const key = `${phase.kind}:${phase.player ?? ""}:${phase.drawn ?? ""}:${phase.id ?? ""}`;
  if (view.remaining === null) {
    timer.key = key;
    timer.total = 1;
    timer.endsAt = 0;
    return;
  }
  const endsAt = performance.now() + view.remaining;
  if (key !== timer.key) {
    timer.key = key;
    timer.total = Math.max(view.remaining, 1);
  }
  timer.endsAt = endsAt;
}

function tickTimer() {
  const bar = $("timer-bar");
  const left = Math.max(0, timer.endsAt - performance.now());
  const turnish = view && ["turn", "target", "discard"].includes(view.phase.kind);
  bar.style.width = turnish ? `${(left / timer.total) * 100}%` : "0%";
  bar.classList.toggle("urgent", turnish && left < 5000);
  requestAnimationFrame(tickTimer);
}

renderSpecials($("lobby-specials"));
requestAnimationFrame(tickTimer);
boot();

