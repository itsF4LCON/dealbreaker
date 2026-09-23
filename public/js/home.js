import { renderSpecials } from "./cards.js";
import { load, save } from "./store.js";

const $ = (id) => document.getElementById(id);
const nameInput = $("name");
const codeInput = $("code");
const error = $("home-error");

nameInput.value = load("db-name") || "";
renderSpecials($("specials"));

function fail(message) {
  error.textContent = message;
  error.hidden = false;
}

$("create-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  error.hidden = true;
  const name = nameInput.value.trim();
  if (!name) return fail("Type your name first.");
  save("db-name", name);
  const button = $("create");
  button.disabled = true;
  button.textContent = "Creating…";
  try {
    const res = await fetch("/api/rooms", { method: "POST" });
    if (!res.ok) throw new Error(await res.text());
    const { code } = await res.json();
    location.href = `/r/${code}`;
  } catch (err) {
    fail(err.message || "Couldn't create a room. Check your connection and try again.");
    button.disabled = false;
    button.textContent = "Create a room";
  }
});

codeInput.addEventListener("input", () => {
  codeInput.value = codeInput.value.toUpperCase().replace(/[^A-Z]/g, "");
});

$("join-form").addEventListener("submit", (e) => {
  e.preventDefault();
  const code = codeInput.value.trim().toUpperCase();
  if (code.length !== 4) return fail("A room code has 4 letters.");
  const name = nameInput.value.trim();
  if (name) save("db-name", name);
  location.href = `/r/${code}`;
});
