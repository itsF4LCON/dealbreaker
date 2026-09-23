export const OUTCOMES = [
  { key: "everyone_draws", short: "ALL +1", about: "Everyone draws 1 card", text: () => "Everyone draws 1" },
  { key: "you_draw", short: "YOU +2", about: "The spinner draws 2", text: (name) => `${name} draws 2` },
  { key: "pick_draw", short: "PICK +2", about: "The spinner picks someone to draw 2", text: (name) => `${name} picks someone to draw 2` },
  { key: "swap_hands", short: "SWAP", about: "The spinner swaps hands with someone", text: (name) => `${name} swaps hands with someone` },
  { key: "skip_next", short: "SKIP", about: "The next player is skipped", text: () => "The next player is skipped" },
  { key: "reverse", short: "REVERSE", about: "The direction of play reverses", text: () => "The direction reverses" },
  { key: "pass_hands", short: "PASS ALL", about: "Everyone passes their hand to the next player", text: () => "Everyone passes their hand on" },
  { key: "throw_away", short: "THROW 1", about: "The spinner throws away a card of their choice", text: (name) => `${name} throws a card away` },
];

const SIZE = 200;
const R = 96;
const SEGMENT = 360 / OUTCOMES.length;

function point(angle, radius) {
  const rad = ((angle - 90) * Math.PI) / 180;
  return [SIZE / 2 + radius * Math.cos(rad), SIZE / 2 + radius * Math.sin(rad)];
}

function wheelSvg() {
  const parts = OUTCOMES.map((o, i) => {
    const [x1, y1] = point(i * SEGMENT, R);
    const [x2, y2] = point((i + 1) * SEGMENT, R);
    const dark = i % 2 === 0;
    const mid = i * SEGMENT + SEGMENT / 2;
    const [tx, ty] = point(mid, R * 0.62);
    return (
      `<path d="M${SIZE / 2} ${SIZE / 2} L${x1} ${y1} A${R} ${R} 0 0 1 ${x2} ${y2} Z" fill="${dark ? "#000" : "#fffaf0"}" stroke="#fffaf0" stroke-width="1"/>` +
      `<text x="${tx}" y="${ty}" fill="${dark ? "#fffaf0" : "#000"}" text-anchor="middle" dominant-baseline="middle" transform="rotate(${mid} ${tx} ${ty})">${o.short}</text>`
    );
  });
  return (
    `<svg class="wheel" viewBox="0 0 ${SIZE} ${SIZE}" role="img" aria-label="Wheel of fortune">` +
    `<circle cx="${SIZE / 2}" cy="${SIZE / 2}" r="${R + 3}" fill="#fffaf0"/>` +
    parts.join("") +
    `<circle cx="${SIZE / 2}" cy="${SIZE / 2}" r="10" fill="#ff3b1f"/></svg>`
  );
}

export function drawWheel(container) {
  container.innerHTML = `<div class="wheel-wrap small"><div class="wheel-pointer"></div>${wheelSvg()}</div>`;
}

export function wheelLegend() {
  return (
    '<ul class="wheel-legend">' +
    OUTCOMES.map((o) => `<li><strong>${o.short}</strong>${o.about}</li>`).join("") +
    "</ul>"
  );
}

export function spinWheel(container, outcomeKey, name) {
  const index = Math.max(0, OUTCOMES.findIndex((o) => o.key === outcomeKey));
  container.innerHTML = `<div class="wheel-wrap"><div class="wheel-pointer"></div>${wheelSvg()}</div><p class="wheel-result"></p>`;
  const wheel = container.querySelector(".wheel");
  const result = container.querySelector(".wheel-result");
  const jitter = (Math.random() - 0.5) * SEGMENT * 0.6;
  const target = 360 * 5 + (360 - (index * SEGMENT + SEGMENT / 2)) + jitter;
  const reveal = () => {
    result.textContent = OUTCOMES[index].text(name);
  };
  if (matchMedia("(prefers-reduced-motion: reduce)").matches) {
    wheel.style.transform = `rotate(${target}deg)`;
    reveal();
    return;
  }
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      wheel.style.transform = `rotate(${target}deg)`;
    }),
  );
  wheel.addEventListener("transitionend", reveal, { once: true });
}
