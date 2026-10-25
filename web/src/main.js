import {
  SPEEDS,
  addPoint,
  clearBattery,
  loadBattery,
  loadPoints,
  loadSettings,
  pointBytes,
  removePoint,
  saveBattery,
  saveSettings,
} from "./storage.js";

const ROM_URL = "/rom/silice-8bc05f48.gba";
const ROM_PATH = "/data/games/silice.gba";
const SAV_PATH = "/data/saves/silice.sav";
const STATE_PATH = "/data/states/silice.ss1";
const STATE_SLOT = 1;

const KEYS = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  KeyX: "a",
  KeyZ: "b",
  KeyA: "l",
  KeyS: "r",
  Enter: "start",
  Backspace: "select",
};

const screen = document.querySelector("#screen");
const stage = document.querySelector("#stage");
const statusEl = document.querySelector("#status");
const controls = document.querySelector("#controls");
const menu = document.querySelector("#menu");
const speedsEl = document.querySelector("#speeds");
const pointsEl = document.querySelector("#points");
const toastEl = document.querySelector("#toast");

const held = new Set();
let emu = null;
let settings = loadSettings();
let batteryAt = loadBattery()?.at ?? null;
let menuOpen = false;
let ready = false;
let bootError = "";
let ignoreBattery = false;
let toastTimer = 0;

function toast(message) {
  toastEl.textContent = message;
  toastEl.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toastEl.classList.remove("show"), 1800);
}

function setStatus(text) {
  if (!text) {
    statusEl.hidden = true;
    return;
  }
  statusEl.hidden = false;
  statusEl.textContent = text;
}

function touchWanted() {
  if (settings.touch === "on") return true;
  if (settings.touch === "off") return false;
  return window.matchMedia("(pointer: coarse), (max-width: 820px)").matches;
}

function applyTouch() {
  const on = touchWanted();
  document.body.dataset.touch = on ? "on" : "off";
  controls.hidden = !on;
}

function fit() {
  const scale = Math.min(stage.clientWidth / 240, stage.clientHeight / 160);
  const size = Math.max(1, scale);
  screen.style.setProperty("--w", `${Math.floor(240 * size)}px`);
  screen.style.setProperty("--h", `${Math.floor(160 * size)}px`);
}

function unlockAudio() {
  const ctx = emu?.SDL2?.audioContext;
  if (ctx && ctx.state === "suspended") ctx.resume();
}

function press(name) {
  if (!emu || menuOpen) return;
  unlockAudio();
  if (!held.has(name)) {
    emu.buttonPress(name);
    held.add(name);
    navigator.vibrate?.(8);
  }
  const button = controls.querySelector(`[data-key="${name}"]`);
  button?.classList.add("is-down");
}

function release(name) {
  if (!emu) return;
  if (held.has(name)) {
    emu.buttonUnpress(name);
    held.delete(name);
  }
  controls.querySelector(`[data-key="${name}"]`)?.classList.remove("is-down");
}

function releaseAll() {
  for (const name of [...held]) release(name);
}

function applyRuntime() {
  if (!emu) return;
  emu.setVolume(settings.volume);
  emu.setFastForwardMultiplier(settings.speed);
  const chip = document.querySelector("#speed-chip");
  chip.hidden = settings.speed === 1;
  chip.textContent = `×${settings.speed}`;
  for (const button of speedsEl.querySelectorAll("button")) {
    button.setAttribute("aria-pressed", String(Number(button.dataset.speed) === settings.speed));
  }
  document.querySelector("#volume").value = String(Math.round(settings.volume * 100));
  document.querySelector("#volume-readout").textContent = `${Math.round(settings.volume * 100)}%`;
  document.querySelector("#touch-mode").value = settings.touch;
}

function updateSettings(patch) {
  settings = { ...settings, ...patch };
  saveSettings(settings);
  applyRuntime();
  applyTouch();
}

function formatWhen(iso) {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function renderBattery() {
  const line = document.querySelector("#battery-line");
  line.textContent = batteryAt
    ? `In-game save kept in this browser · ${formatWhen(batteryAt)}`
    : "In-game saves are kept in this browser.";
}

function renderPoints() {
  const points = loadPoints();
  pointsEl.replaceChildren();
  if (points.length === 0) {
    const empty = document.createElement("li");
    empty.className = "empty";
    empty.textContent = "No save points yet.";
    pointsEl.append(empty);
    return;
  }
  for (const point of points) {
    const item = document.createElement("li");
    const meta = document.createElement("div");
    meta.className = "meta";
    const name = document.createElement("strong");
    name.textContent = point.name;
    const when = document.createElement("span");
    when.textContent = formatWhen(point.at);
    meta.append(name, when);
    const load = document.createElement("button");
    load.type = "button";
    load.textContent = "Load";
    load.addEventListener("click", () => loadPoint(point));
    const del = document.createElement("button");
    del.type = "button";
    del.textContent = "Delete";
    del.addEventListener("click", () => {
      if (!del.classList.contains("armed")) {
        del.classList.add("armed");
        del.textContent = "Confirm";
        setTimeout(() => {
          del.classList.remove("armed");
          del.textContent = "Delete";
        }, 2500);
        return;
      }
      removePoint(point.id);
      renderPoints();
      toast("Save point deleted");
    });
    item.append(meta, load, del);
    pointsEl.append(item);
  }
}

function copyBytes(view) {
  const copy = new Uint8Array(view.byteLength);
  copy.set(view);
  return copy;
}

function persistBattery() {
  if (ignoreBattery || !emu) return;
  try {
    const sav = emu.getSave();
    if (!sav || sav.byteLength === 0) return;
    batteryAt = saveBattery(copyBytes(sav));
    renderBattery();
  } catch (error) {
    toast(error?.name === "QuotaExceededError" ? "Browser storage is full" : "Could not store the save");
  }
}

function writeBatteryFile() {
  const battery = loadBattery();
  try {
    emu.FS.unlink(SAV_PATH);
  } catch {
    /* no previous file */
  }
  if (battery?.bytes?.byteLength) {
    emu.FS.writeFile(SAV_PATH, battery.bytes);
    batteryAt = battery.at;
  } else {
    batteryAt = null;
  }
}

function restartRom() {
  ignoreBattery = true;
  try {
    emu.quitGame();
  } catch {
    /* core already stopped */
  }
  writeBatteryFile();
  const loaded = emu.loadGame(ROM_PATH);
  ignoreBattery = false;
  if (!loaded) {
    toast("Could not restart");
    return false;
  }
  applyRuntime();
  return true;
}

function stateFile() {
  const names = emu.FS.readdir("/data/states").filter((name) => name !== "." && name !== "..");
  const match = names.find((name) => name.endsWith(".ss1") || name.includes("ss1"));
  if (!match) return null;
  return `/data/states/${match}`;
}

async function capturePoint(name) {
  if (!emu || !ready) return;
  const wasPaused = menuOpen;
  if (wasPaused) emu.resumeGame();
  const ok = emu.saveState(STATE_SLOT);
  if (wasPaused) emu.pauseGame();
  if (!ok) {
    toast("Could not capture a save point");
    return;
  }
  const path = stateFile() || STATE_PATH;
  let bytes;
  try {
    bytes = copyBytes(emu.FS.readFile(path));
  } catch {
    toast("Could not read the save point");
    return;
  }
  try {
    await addPoint(name, bytes);
  } catch (error) {
    toast(error?.name === "QuotaExceededError" ? "Browser storage is full" : "Could not store the save point");
    return;
  }
  renderPoints();
  toast("Save point stored");
}

async function loadPoint(point) {
  if (!emu || !ready) return;
  let bytes;
  try {
    bytes = await pointBytes(point);
  } catch {
    toast("Could not read that save point");
    return;
  }
  const path = stateFile() || STATE_PATH;
  emu.FS.writeFile(path, bytes);
  const wasPaused = menuOpen;
  if (wasPaused) emu.resumeGame();
  const ok = emu.loadState(STATE_SLOT);
  if (wasPaused) emu.pauseGame();
  if (!ok) {
    toast("Could not load that save point");
    return;
  }
  persistBattery();
  toast(`Loaded ${point.name}`);
  menu.close();
}

function bindHolds() {
  for (const button of controls.querySelectorAll("[data-key]")) {
    const name = button.dataset.key;
    let down = false;
    const start = (event) => {
      event.preventDefault();
      button.setPointerCapture?.(event.pointerId);
      down = true;
      press(name);
    };
    const stop = () => {
      if (!down) return;
      down = false;
      release(name);
    };
    button.addEventListener("pointerdown", start);
    button.addEventListener("pointerup", stop);
    button.addEventListener("pointercancel", stop);
    button.addEventListener("lostpointercapture", stop);
    button.addEventListener("contextmenu", (event) => event.preventDefault());
  }
}

function typingTarget(target) {
  return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement;
}

function onKeyDown(event) {
  if (event.repeat && KEYS[event.code]) {
    event.preventDefault();
    return;
  }
  if (event.code === "Escape") {
    if (menuOpen) menu.close();
    else if (document.fullscreenElement) document.exitFullscreen();
    return;
  }
  if (menuOpen || typingTarget(event.target)) return;
  if (event.code === "KeyF") {
    event.preventDefault();
    toggleFullscreen();
    return;
  }
  if (event.code === "KeyM") {
    event.preventDefault();
    menu.showModal();
    return;
  }
  if (event.code === "F5") {
    event.preventDefault();
    capturePoint("Quick save");
    return;
  }
  if (event.code === "F8") {
    event.preventDefault();
    emu?.toggleRewind(true);
    return;
  }
  if (event.code === "F9") {
    event.preventDefault();
    const [latest] = loadPoints();
    if (latest) loadPoint(latest);
    else toast("No save point to load");
    return;
  }
  if (/^Digit[1-4]$/.test(event.code)) {
    updateSettings({ speed: SPEEDS[Number(event.code.slice(5)) - 1] });
    return;
  }
  const name = KEYS[event.code];
  if (!name) return;
  event.preventDefault();
  press(name);
}

function onKeyUp(event) {
  if (event.code === "F8") {
    emu?.toggleRewind(false);
    return;
  }
  const name = KEYS[event.code];
  if (name) release(name);
}

function toggleFullscreen() {
  if (document.fullscreenElement) {
    document.exitFullscreen();
    return;
  }
  const root = document.documentElement;
  const request = root.requestFullscreen || root.webkitRequestFullscreen;
  request?.call(root);
}

function snapshot() {
  return {
    mode: bootError ? "error" : menuOpen ? "menu" : ready ? "play" : "loading",
    ready,
    error: bootError,
    speed: settings.speed,
    volume: settings.volume,
    touch: document.body.dataset.touch || "off",
    menuOpen,
    held: [...held],
    batteryAt,
    points: loadPoints().map((point) => ({ id: point.id, name: point.name, at: point.at })),
    coordinateSystem: "GBA screen 240x160, origin top-left, +x right, +y down",
  };
}

function wireMenu() {
  for (const speed of SPEEDS) {
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.speed = String(speed);
    button.textContent = speed === 1 ? "Normal" : `×${speed}`;
    button.addEventListener("click", () => updateSettings({ speed }));
    speedsEl.append(button);
  }
  document.querySelector("#volume").addEventListener("input", (event) => {
    updateSettings({ volume: Number(event.target.value) / 100 });
  });
  document.querySelector("#touch-mode").addEventListener("change", (event) => {
    updateSettings({ touch: event.target.value });
  });
  document.querySelector("#save-point").addEventListener("click", () => {
    const input = document.querySelector("#point-name");
    capturePoint(input.value);
    input.value = "";
  });
  document.querySelector("#fullscreen").addEventListener("click", toggleFullscreen);
  document.querySelector("#reload-rom").addEventListener("click", () => {
    if (!emu) return;
    if (restartRom()) toast("Reloaded");
    menu.close();
  });
  const erase = document.querySelector("#erase-save");
  erase.addEventListener("click", () => {
    if (!erase.classList.contains("armed")) {
      erase.classList.add("armed");
      erase.textContent = "Confirm erase";
      setTimeout(() => {
        erase.classList.remove("armed");
        erase.textContent = "Erase in-game save";
      }, 2500);
      return;
    }
    clearBattery();
    batteryAt = null;
    if (emu && restartRom()) toast("In-game save erased");
    renderBattery();
    menu.close();
  });
  document.querySelector("#open-menu").addEventListener("click", () => menu.showModal());
  menu.addEventListener("close", () => {
    menuOpen = false;
    emu?.resumeGame();
    emu?.resumeAudio();
  });
  menu.addEventListener("cancel", (event) => {
    event.preventDefault();
    menu.close();
  });
  const observer = new MutationObserver(() => {
    const open = menu.open;
    if (open === menuOpen) return;
    menuOpen = open;
    if (open) {
      releaseAll();
      emu?.pauseGame();
      renderPoints();
      renderBattery();
    }
  });
  observer.observe(menu, { attributes: true, attributeFilter: ["open"] });
}

async function boot() {
  wireMenu();
  bindHolds();
  applyTouch();
  fit();
  renderBattery();
  renderPoints();
  applyRuntime();
  window.addEventListener("resize", () => {
    fit();
    applyTouch();
  });
  window.matchMedia("(pointer: coarse), (max-width: 820px)").addEventListener("change", applyTouch);
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("keyup", onKeyUp);
  window.addEventListener("blur", () => {
    releaseAll();
    emu?.toggleRewind(false);
  });
  window.addEventListener("pointerdown", unlockAudio);
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") persistBattery();
  });
  window.addEventListener("pagehide", persistBattery);

  if (!crossOriginIsolated) {
    bootError = "This page needs cross-origin isolation to run the emulator.";
    setStatus(bootError);
    return;
  }

  const mGBA = (await import(/* @vite-ignore */ "/vendor/mgba.js")).default;
  emu = await mGBA({ canvas: screen });
  emu.toggleInput(false);
  await emu.FSInit();
  emu.setCoreSettings({
    autoSaveStateEnable: false,
    restoreAutoSaveStateOnLoad: false,
    rewindEnable: true,
    rewindBufferCapacity: 600,
    rewindBufferInterval: 1,
    showFpsCounter: false,
  });
  emu.addCoreCallbacks({
    saveDataUpdatedCallback: () => persistBattery(),
  });

  const response = await fetch(ROM_URL);
  if (!response.ok) throw new Error(`ROM request failed (${response.status})`);
  const rom = new Uint8Array(await response.arrayBuffer());
  emu.FS.writeFile(ROM_PATH, rom);
  writeBatteryFile();
  const loaded = emu.loadGame(ROM_PATH);
  if (!loaded) throw new Error("mGBA could not start the ROM");
  ready = true;
  setStatus("");
  applyRuntime();
  fit();
  unlockAudio();
}

window.render_game_to_text = () => JSON.stringify(snapshot());
window.advanceTime = (ms) =>
  new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
window.__silice = {
  press,
  release,
  snapshot,
};

boot().catch((error) => {
  bootError = error?.message || String(error);
  setStatus(bootError);
  console.error(error);
});

new ResizeObserver(fit).observe(stage);
