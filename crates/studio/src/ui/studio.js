// Frame pump (plan: "Frame transport"). Each `<canvas data-pane=NAME>` runs one request loop on
// `/frames/NAME?after=SEQ`, a long poll: 200 carries the next raw RGBA frame (240x160x4) and its
// `x-seq` as soon as the pane publishes it, 204 means nothing new for a while. One request in
// flight per canvas, so stale frames never queue. `<span data-fps=NAME>` shows the delivered
// fps; `/stats/NAME?...` reports to Rust every 500 ms (used by STUDIO_SMOKE_SECS).
(() => {
  const W = 240, H = 160;
  const panes = new Map();
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

  function stats(name) {
    let s = panes.get(name);
    if (!s) {
      s = { drawn: 0, rtt: 0, age: 0, max: 0, last: 0, lastT: performance.now() };
      panes.set(name, s);
    }
    return s;
  }

  async function run(canvas) {
    const name = canvas.dataset.pane;
    const ctx = canvas.getContext("2d");
    const s = stats(name);
    let seq = 0; // a new canvas (e.g. after a tab switch) wants the current frame at once
    while (canvas.isConnected) {
      const t0 = performance.now();
      try {
        const r = await fetch(`/frames/${name}?after=${seq}`, { cache: "no-store" });
        if (r.status !== 200) {
          if (r.status !== 204) await sleep(250);
          continue;
        }
        const buf = await r.arrayBuffer();
        if (buf.byteLength !== W * H * 4) continue;
        seq = Number(r.headers.get("x-seq"));
        ctx.putImageData(new ImageData(new Uint8ClampedArray(buf), W, H), 0, 0);
        // Latency: publish -> served (`x-age-us`), then served -> drawn, bounded by the fetch
        // time minus the long-poll wait (`x-wait-us`).
        const now = performance.now();
        const age = Number(r.headers.get("x-age-us") || 0) / 1000;
        const wait = Number(r.headers.get("x-wait-us") || 0) / 1000;
        const served = Math.max(0, now - t0 - wait);
        s.drawn += 1;
        s.rtt += served;
        s.age += age;
        s.max = Math.max(s.max, age + served);
      } catch (e) {
        await sleep(250);
      }
    }
  }

  const running = new WeakSet();
  setInterval(() => {
    document.querySelectorAll("canvas[data-pane]").forEach((c) => {
      if (!running.has(c)) {
        running.add(c);
        run(c);
      }
    });
  }, 50);

  setInterval(() => {
    const now = performance.now();
    for (const [name, s] of panes) {
      const fps = ((s.drawn - s.last) * 1000) / (now - s.lastT);
      s.last = s.drawn;
      s.lastT = now;
      document.querySelectorAll(`[data-fps="${name}"]`).forEach((e) => {
        e.textContent = `${fps.toFixed(1)} fps`;
      });
      const q = `t=${now.toFixed(1)}&drawn=${s.drawn}&rtt=${s.rtt.toFixed(3)}` +
        `&age=${s.age.toFixed(3)}&max=${s.max.toFixed(3)}`;
      s.max = 0;
      fetch(`/stats/${name}?${q}`, { cache: "no-store" }).catch(() => {});
    }
  }, 500);
})();
