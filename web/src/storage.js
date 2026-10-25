const SETTINGS_KEY = "silice.settings.v1";
const BATTERY_KEY = "silice.battery.v1";
const POINTS_KEY = "silice.points.v1";

const SPEEDS = [1, 2, 4, 8];

function toB64(bytes) {
  let raw = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) {
    raw += String.fromCharCode(...bytes.subarray(i, i + chunk));
  }
  return btoa(raw);
}

function fromB64(b64) {
  const raw = atob(b64);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
  return out;
}

async function compress(bytes) {
  if (typeof CompressionStream !== "function") {
    return { enc: "raw", b64: toB64(bytes) };
  }
  const stream = new Blob([bytes]).stream().pipeThrough(new CompressionStream("gzip"));
  const packed = new Uint8Array(await new Response(stream).arrayBuffer());
  return { enc: "gzip", b64: toB64(packed) };
}

async function decompress(entry) {
  const packed = fromB64(entry.b64);
  if (entry.enc !== "gzip" || typeof DecompressionStream !== "function") return packed;
  const stream = new Blob([packed]).stream().pipeThrough(new DecompressionStream("gzip"));
  return new Uint8Array(await new Response(stream).arrayBuffer());
}

function readJson(key, fallback) {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : fallback;
  } catch {
    return fallback;
  }
}

export function loadSettings() {
  const saved = readJson(SETTINGS_KEY, {});
  const speed = SPEEDS.includes(saved.speed) ? saved.speed : 1;
  const volume = Number.isFinite(saved.volume) ? Math.min(1, Math.max(0, saved.volume)) : 1;
  const touch = ["auto", "on", "off"].includes(saved.touch) ? saved.touch : "auto";
  return { speed, volume, touch };
}

export function saveSettings(settings) {
  localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
}

export function loadBattery() {
  const saved = readJson(BATTERY_KEY, null);
  if (!saved?.b64) return null;
  try {
    return { at: saved.at || null, bytes: fromB64(saved.b64) };
  } catch {
    return null;
  }
}

export function saveBattery(bytes) {
  const at = new Date().toISOString();
  localStorage.setItem(BATTERY_KEY, JSON.stringify({ at, b64: toB64(bytes) }));
  return at;
}

export function clearBattery() {
  localStorage.removeItem(BATTERY_KEY);
}

export function loadPoints() {
  const points = readJson(POINTS_KEY, []);
  return Array.isArray(points) ? points : [];
}

export async function pointBytes(point) {
  return decompress(point);
}

export async function addPoint(name, bytes) {
  const points = loadPoints();
  const packed = await compress(bytes);
  const point = {
    id: crypto.randomUUID(),
    name: name.trim() || "Save point",
    at: new Date().toISOString(),
    enc: packed.enc,
    b64: packed.b64,
  };
  points.unshift(point);
  localStorage.setItem(POINTS_KEY, JSON.stringify(points));
  return point;
}

export function removePoint(id) {
  const points = loadPoints().filter((point) => point.id !== id);
  localStorage.setItem(POINTS_KEY, JSON.stringify(points));
  return points;
}

export { SPEEDS };
