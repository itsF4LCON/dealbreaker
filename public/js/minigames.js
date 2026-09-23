import { cardElement, rankLabel } from "./cards.js";

const STOP_CLOCK_TARGET = 5000;
const STOP_CLOCK_VISIBLE_MS = 2000;
const STOP_CLOCK_GIVE_UP_MS = 11000;
const QUICK_DRAW_GIVE_UP_MS = 2500;
const QUICK_DRAW_MIN_MS = 50;
const MASH_MS = 5000;
const HIGH_CARD_AUTO_FLIP_MS = 6000;
const CASH_OUT_DOUBLING_MS = 3000;
const STACK_LEVELS = 5;
const STACK_START = { left: 20, right: 80 };
const STACK_BLOCK_PX = 36;
const STACK_GIVE_UP_MS = 13000;

export const GAMES = {
  stop_clock: { title: "Stop the clock", howto: "The clock disappears after 2 seconds. Stop it at exactly 5.00." },
  quick_draw: { title: "Quick draw", howto: "Wait for the flash, then tap as fast as you can. Too early and you lose." },
  mash: { title: "Mash", howto: "Tap as fast as you can for 5 seconds. Most taps wins." },
  high_card: { title: "High card", howto: "Flip your card. The lowest card loses. Aces are high." },
  cash_out: { title: "Cash out", howto: "The multiplier climbs until it crashes. Cash out as high as you dare." },
  stacker: { title: "Stacker", howto: "Drop 5 cards on the stack. Overhang gets cut off. Widest stack wins." },
};

function highCardLabel(rank) {
  return rank === 14 ? "A" : rankLabel(rank);
}

export function formatResult(game, value) {
  const none = value === null || value === undefined;
  switch (game) {
    case "stop_clock":
      if (none) return "never stopped";
      return `${(value / 1000).toFixed(2)} s, ${(Math.abs(value - STOP_CLOCK_TARGET) / 1000).toFixed(2)} off`;
    case "quick_draw":
      return none ? "false start" : `${value} ms`;
    case "mash":
      return none ? "didn't play" : `${value} taps`;
    case "high_card":
      return none ? "no card" : highCardLabel(value);
    case "cash_out":
      return none ? "crashed" : `x${(value / 100).toFixed(2)}`;
    case "stacker":
      if (none) return "didn't finish";
      return value === 0 ? "fell off" : `${Math.round(value / 6)}% left`;
    default:
      return String(value);
  }
}

export function runMinigame({ area, game, startsIn, param, playing, signal }) {
  return new Promise((resolve) => {
    const cleanups = [];
    let finished = false;
    const finish = (value) => {
      if (finished) return;
      finished = true;
      cleanups.forEach((fn) => fn());
      resolve(value);
    };
    const later = (fn, ms) => {
      const t = setTimeout(fn, ms);
      cleanups.push(() => clearTimeout(t));
    };
    const listen = (target, type, fn) => {
      target.addEventListener(type, fn);
      cleanups.push(() => target.removeEventListener(type, fn));
    };
    const alive = () => !finished;
    signal.addEventListener("abort", () => finish(undefined), { once: true });

    const startAt = performance.now() + Math.max(0, startsIn);
    const count = () => {
      if (finished) return;
      const left = startAt - performance.now();
      if (left <= 0) {
        if (!playing) return finish(undefined);
        const run = { stop_clock: stopClock, quick_draw: quickDraw, mash, high_card: highCard, cash_out: cashOut, stacker }[game];
        run({ area, param, finish, later, listen, alive });
        return;
      }
      area.innerHTML = `<span class="mg-count">${Math.ceil(left / 1000)}</span>`;
      later(count, Math.min(left, left % 1000 || 1000));
    };
    count();
  });
}

function onTap(listen, target, fn) {
  listen(target, "pointerdown", (e) => {
    e.preventDefault();
    fn();
  });
  listen(document, "keydown", (e) => {
    if (e.repeat) return;
    if (e.code === "Space" || e.code === "Enter") {
      e.preventDefault();
      fn();
    }
  });
}

function stopClock({ area, finish, listen }) {
  area.innerHTML =
    '<div class="mg-play">' +
    '<div class="mg-display" aria-live="off">0.00</div>' +
    '<p class="mg-note">Stop it at 5.00</p>' +
    '<button type="button" class="mg-button">STOP</button>' +
    "</div>";
  const display = area.querySelector(".mg-display");
  const button = area.querySelector(".mg-button");
  const note = area.querySelector(".mg-note");
  const t0 = performance.now();
  let stopped = false;

  const stop = (value) => {
    if (stopped) return;
    stopped = true;
    button.disabled = true;
    display.textContent = value === null ? "--" : (value / 1000).toFixed(2);
    note.textContent =
      value === null ? "Too late." : `${(Math.abs(value - STOP_CLOCK_TARGET) / 1000).toFixed(2)} s off. Waiting for the others…`;
    finish(value);
  };
  const frame = () => {
    if (stopped) return;
    const elapsed = performance.now() - t0;
    display.textContent = elapsed < STOP_CLOCK_VISIBLE_MS ? (elapsed / 1000).toFixed(2) : "?.??";
    if (elapsed > STOP_CLOCK_GIVE_UP_MS) return stop(null);
    requestAnimationFrame(frame);
  };
  onTap(listen, button, () => stop(Math.max(1, Math.round(performance.now() - t0))));
  requestAnimationFrame(frame);
}

function quickDraw({ area, param, finish, later, listen }) {
  area.innerHTML = '<button type="button" class="mg-pad">Wait for it…</button>';
  const pad = area.querySelector(".mg-pad");
  let flashedAt = null;
  let done = false;

  const end = (value, text) => {
    if (done) return;
    done = true;
    pad.classList.remove("go");
    pad.disabled = true;
    pad.textContent = text;
    finish(value);
  };
  later(() => {
    if (done) return;
    pad.classList.add("go");
    pad.textContent = "TAP!";
    requestAnimationFrame(() => {
      flashedAt = performance.now();
    });
    later(() => end(null, "Too slow."), QUICK_DRAW_GIVE_UP_MS);
  }, param ?? 2000);
  onTap(listen, pad, () => {
    if (flashedAt === null) return end(null, "Too early!");
    const ms = Math.round(performance.now() - flashedAt);
    if (ms < QUICK_DRAW_MIN_MS) return end(null, "Too early!");
    end(ms, `${ms} ms`);
  });
}

function mash({ area, finish, later, listen, alive }) {
  area.innerHTML =
    '<div class="mg-play">' +
    '<div class="mg-display">0</div>' +
    '<div class="mg-meter"><div class="mg-meter-bar"></div></div>' +
    '<button type="button" class="mg-pad go">TAP!</button>' +
    "</div>";
  const display = area.querySelector(".mg-display");
  const bar = area.querySelector(".mg-meter-bar");
  const pad = area.querySelector(".mg-pad");
  const t0 = performance.now();
  let taps = 0;
  let running = true;

  onTap(listen, pad, () => {
    if (!running) return;
    taps += 1;
    display.textContent = taps;
  });
  const frame = () => {
    if (!running || !alive()) return;
    bar.style.width = `${Math.max(0, 1 - (performance.now() - t0) / MASH_MS) * 100}%`;
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  later(() => {
    running = false;
    bar.style.width = "0%";
    pad.classList.remove("go");
    pad.disabled = true;
    pad.textContent = `${taps} taps. Waiting for the others…`;
    finish(taps);
  }, MASH_MS);
}

function highCard({ area, param, finish, later, listen }) {
  area.innerHTML =
    '<div class="mg-play">' +
    '<button type="button" class="hc-card" aria-label="Flip your card"><span class="card back big"></span></button>' +
    '<p class="mg-note">Tap your card to flip it</p>' +
    "</div>";
  const slot = area.querySelector(".hc-card");
  const note = area.querySelector(".mg-note");
  let flipped = false;

  const flip = () => {
    if (flipped) return;
    flipped = true;
    const rank = param ?? 2;
    const suit = ["spades", "hearts", "clubs", "diamonds"][rank % 4];
    const face = cardElement({ id: -99, face: { kind: "normal", suit, rank: rank === 14 ? 1 : rank } }, "span");
    face.classList.add("big", "flip-in");
    slot.replaceChildren(face);
    slot.disabled = true;
    note.textContent = `You got ${highCardLabel(rank)}. Waiting for the others…`;
    finish(rank);
  };
  onTap(listen, slot, flip);
  later(flip, HIGH_CARD_AUTO_FLIP_MS);
}

function cashOut({ area, param, finish, listen, alive }) {
  area.innerHTML =
    '<div class="mg-play">' +
    '<div class="mg-display">x1.00</div>' +
    '<p class="mg-note">Cash out before it crashes</p>' +
    '<button type="button" class="mg-button">CASH OUT</button>' +
    "</div>";
  const display = area.querySelector(".mg-display");
  const note = area.querySelector(".mg-note");
  const button = area.querySelector(".mg-button");
  const crash = param ?? 300;
  const t0 = performance.now();
  const multiplier = () => Math.floor(100 * 2 ** ((performance.now() - t0) / CASH_OUT_DOUBLING_MS));
  let cashed = null;
  let crashed = false;

  const frame = () => {
    const m = multiplier();
    if (m >= crash) {
      crashed = true;
      display.textContent = `x${(crash / 100).toFixed(2)}`;
      display.classList.remove("cashed");
      display.classList.add("crashed");
      button.disabled = true;
      if (cashed === null) {
        note.textContent = "Crashed before you cashed out.";
        finish(null);
      } else {
        note.textContent = `You cashed out at x${(cashed / 100).toFixed(2)}. It crashed here.`;
      }
      return;
    }
    display.textContent = `x${(m / 100).toFixed(2)}`;
    if (alive() || cashed !== null) requestAnimationFrame(frame);
  };
  onTap(listen, button, () => {
    if (cashed !== null || crashed) return;
    const m = multiplier();
    if (m >= crash) return;
    cashed = m;
    button.disabled = true;
    display.classList.add("cashed");
    note.textContent = `Cashed out at x${(m / 100).toFixed(2)}. Waiting for the others…`;
    finish(m);
  });
  requestAnimationFrame(frame);
}

function stacker({ area, finish, later, listen, alive }) {
  area.innerHTML =
    '<div class="mg-play">' +
    '<div class="stack-box" role="button" aria-label="Drop the card"></div>' +
    '<p class="mg-note">Tap to drop. 5 cards to go.</p>' +
    "</div>";
  const box = area.querySelector(".stack-box");
  const note = area.querySelector(".mg-note");
  let below = { ...STACK_START };
  let level = 1;
  let x = 0;
  let dir = 1;
  let last = performance.now();
  let done = false;

  const block = (left, width, lvl, cls = "") => {
    const el = document.createElement("div");
    el.className = `stack-block ${cls}`;
    el.style.left = `${left}%`;
    el.style.width = `${width}%`;
    el.style.bottom = `${lvl * STACK_BLOCK_PX}px`;
    box.append(el);
    return el;
  };
  block(below.left, below.right - below.left, 0);
  let moving = block(0, below.right - below.left, level, "moving");

  const end = (value, text) => {
    if (done) return;
    done = true;
    note.textContent = text;
    finish(value);
  };
  const frame = (now) => {
    if (done || !alive()) return;
    const width = below.right - below.left;
    const speed = 45 + 22 * level;
    x += dir * speed * ((now - last) / 1000);
    last = now;
    if (x <= 0) (x = 0), (dir = 1);
    if (x >= 100 - width) (x = 100 - width), (dir = -1);
    moving.style.left = `${x}%`;
    requestAnimationFrame(frame);
  };
  const drop = () => {
    if (done) return;
    const left = Math.max(x, below.left);
    const right = Math.min(x + (below.right - below.left), below.right);
    if (right - left <= 0.5) {
      moving.classList.add("fall");
      return end(0, "Missed! The stack fell over.");
    }
    moving.remove();
    block(left, right - left, level);
    below = { left, right };
    level += 1;
    const width = right - left;
    if (level > STACK_LEVELS) {
      const value = Math.round(width * 10);
      return end(value, `${Math.round(value / 6)}% left. Waiting for the others…`);
    }
    note.textContent = `${STACK_LEVELS - level + 1} to go`;
    x = dir === 1 ? 100 - width : 0;
    moving = block(x, width, level, "moving");
  };
  onTap(listen, box, drop);
  later(() => end(null, "Out of time."), STACK_GIVE_UP_MS);
  requestAnimationFrame((now) => {
    last = now;
    frame(now);
  });
}
