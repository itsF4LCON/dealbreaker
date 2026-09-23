const EMOJIS = ["😂", "🔥", "💀", "🤡", "😭", "👋"];
const FLOAT_MS = 2600;

let ctx = null;
let lastReaction = null;
let calloutEndsAt = 0;

const $ = (id) => document.getElementById(id);

export function initSpice(context) {
  ctx = context;
  const row = $("react-row");
  row.innerHTML = EMOJIS.map((e, i) => `<button type="button" data-emoji="${i}" aria-label="React with ${e}">${e}</button>`).join("");
  row.addEventListener("click", (e) => {
    const button = e.target.closest("button");
    if (button) ctx.send({ t: "react", emoji: Number(button.dataset.emoji) });
  });
  const toggle = $("react-toggle");
  toggle.addEventListener("click", () => {
    const open = row.hidden;
    row.hidden = !open;
    toggle.setAttribute("aria-expanded", String(open));
  });
  $("callout").addEventListener("click", (e) => {
    const button = e.target.closest("button");
    if (!button) return;
    button.disabled = true;
    ctx.send({ t: button.dataset.action });
  });
}

export function renderSpice(view) {
  $("react").hidden = false;
  showReactions(view);
  renderCallout(view);
}

export function tickSpice() {
  const box = $("callout");
  if (!box.hidden && performance.now() > calloutEndsAt) box.hidden = true;
}

function showReactions(view) {
  const newest = view.reactions.length ? view.reactions[view.reactions.length - 1].seq : 0;
  if (lastReaction !== null) {
    for (const r of view.reactions) if (r.seq > lastReaction) floatReaction(r);
  }
  lastReaction = Math.max(lastReaction ?? 0, newest);
}

function floatReaction(reaction) {
  const el = document.createElement("div");
  el.className = "float";
  el.style.left = `${15 + Math.random() * 70}%`;
  el.innerHTML = `<span class="float-emoji">${EMOJIS[reaction.emoji] ?? ""}</span><span class="float-name">${ctx.escapeHTML(ctx.nameOf(reaction.player))}</span>`;
  $("floats").append(el);
  setTimeout(() => el.remove(), FLOAT_MS);
}

function renderCallout(view) {
  const box = $("callout");
  const callout = view.callout;
  if (!callout) {
    box.hidden = true;
    return;
  }
  calloutEndsAt = performance.now() + callout.remaining;
  const name = ctx.escapeHTML(ctx.nameOf(callout.player));
  box.innerHTML =
    callout.player === view.you
      ? '<button type="button" class="callout-me" data-action="call">DEALBREAKER!</button><span>You have 1 card. Tap it before someone catches you.</span>'
      : `<button type="button" class="callout-catch" data-action="catch">CAUGHT ${name.toUpperCase()}!</button><span>${name} has 1 card left</span>`;
  box.hidden = false;
}
