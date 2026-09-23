const STOP_CLOCK_TARGET = 5000;
const STOP_CLOCK_VISIBLE_MS = 2000;
const STOP_CLOCK_GIVE_UP_MS = 11000;
const QUICK_DRAW_GIVE_UP_MS = 2500;
const QUICK_DRAW_MIN_MS = 50;

export const GAMES = {
  stop_clock: {
    title: "Stop the clock",
    howto: "The clock disappears after 2 seconds. Stop it at exactly 5.00.",
  },
  quick_draw: {
    title: "Quick draw",
    howto: "Wait for the flash, then tap as fast as you can. Too early and you lose.",
  },
};

export function formatResult(game, value) {
  if (value === null || value === undefined) return game === "stop_clock" ? "never stopped" : "false start";
  if (game === "stop_clock") {
    const off = Math.abs(value - STOP_CLOCK_TARGET) / 1000;
    return `${(value / 1000).toFixed(2)} s, ${off.toFixed(2)} off`;
  }
  return `${value} ms`;
}

export function runMinigame({ area, game, startsIn, delay, playing, signal }) {
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
    signal.addEventListener("abort", () => finish(undefined), { once: true });

    const startAt = performance.now() + Math.max(0, startsIn);
    const count = () => {
      if (finished) return;
      const left = startAt - performance.now();
      if (left <= 0) {
        if (!playing) return finish(undefined);
        const run = game === "stop_clock" ? stopClock : quickDraw;
        run({ area, delay, finish, later, listen });
        return;
      }
      area.innerHTML = `<span class="mg-count">${Math.ceil(left / 1000)}</span>`;
      later(count, Math.min(left, (left % 1000) || 1000));
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

  const frame = () => {
    if (stopped) return;
    const elapsed = performance.now() - t0;
    display.textContent = elapsed < STOP_CLOCK_VISIBLE_MS ? (elapsed / 1000).toFixed(2) : "?.??";
    if (elapsed > STOP_CLOCK_GIVE_UP_MS) return stop(null);
    requestAnimationFrame(frame);
  };
  const stop = (value) => {
    if (stopped) return;
    stopped = true;
    button.disabled = true;
    display.textContent = value === null ? "--" : (value / 1000).toFixed(2);
    note.textContent = value === null ? "Too late." : `${(Math.abs(value - STOP_CLOCK_TARGET) / 1000).toFixed(2)} s off. Waiting for the others…`;
    finish(value);
  };
  onTap(listen, button, () => stop(Math.max(1, Math.round(performance.now() - t0))));
  requestAnimationFrame(frame);
}

function quickDraw({ area, delay, finish, later, listen }) {
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
  }, delay ?? 2000);
  onTap(listen, pad, () => {
    if (flashedAt === null) return end(null, "Too early!");
    const ms = Math.round(performance.now() - flashedAt);
    if (ms < QUICK_DRAW_MIN_MS) return end(null, "Too early!");
    end(ms, `${ms} ms`);
  });
}
