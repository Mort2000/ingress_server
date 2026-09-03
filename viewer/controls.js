// controls.js — playback state machine + input wiring. Jumping to any
// round is direct array indexing (O(1), D89): state {round, playing,
// speed, view}; space play/pause, arrows step, Home/End jump, +/- speed,
// V cycles the view.
const Controls = {
  state: { round: 0, playing: false, speed: 500, view: "god" },
  timer: null,

  init() {
    // Default view: God in a dev export, otherwise the one embedded
    // faction (D90) — the other faction label renders disabled below.
    if (!D.header.dev) {
      this.state.view = Filter.embedded[0] || "public";
    }
    this.wireButtons();
    this.wireSlider();
    this.wireViews();
    this.wireKeyboard();
    this.refresh();
  },

  wireButtons() {
    document.getElementById("btn-play").addEventListener("click", () => this.togglePlay());
    document.getElementById("btn-back").addEventListener("click", () => this.step(-1));
    document.getElementById("btn-fwd").addEventListener("click", () => this.step(1));
    document.getElementById("btn-home").addEventListener("click", () => this.jump(0));
    document.getElementById("btn-end").addEventListener("click", () => this.jump(D.roundCount));
    document.getElementById("speed").addEventListener("change", (event) => {
      this.setSpeed(Number(event.target.value));
    });
  },

  wireSlider() {
    const slider = document.getElementById("slider");
    slider.max = String(D.roundCount);
    slider.addEventListener("input", () => this.jump(Number(slider.value)));

    // Checkpoint ticks: display-only marks proving archive integrity
    // (the IARP checkpoints verified at export; seeking never needs them).
    const ticks = document.getElementById("ticks");
    const n = D.roundCount || 1;
    for (const round of D.header.checkpoints) {
      const tick = document.createElement("div");
      tick.className = "tick";
      tick.title = "IARP checkpoint after round " + round;
      tick.style.left = ((round / n) * 100).toFixed(2) + "%";
      ticks.appendChild(tick);
    }
  },

  wireViews() {
    const note = document.getElementById("view-note");
    document.querySelectorAll("#views button").forEach((button) => {
      const view = button.dataset.view;
      const usable = view === "public" || view === "god" || Filter.embedded.includes(view);
      if (!usable) {
        button.disabled = true;
        button.title =
          "not embedded in this export (R-VIS): this faction's private data does not exist in the file";
      }
      button.addEventListener("click", () => this.setView(view));
    });
    if (D.header.dev) {
      note.textContent = "dev export: all four views active (both factions embedded)";
    } else {
      note.textContent =
        "faction export: the other faction view is disabled — its private data is not in this file";
    }
  },

  wireKeyboard() {
    document.addEventListener("keydown", (event) => {
      if (event.target && /^(INPUT|SELECT|TEXTAREA)$/.test(event.target.tagName)) return;
      switch (event.key) {
        case " ":
          event.preventDefault();
          this.togglePlay();
          break;
        case "ArrowLeft":
          event.preventDefault();
          this.step(-1);
          break;
        case "ArrowRight":
          event.preventDefault();
          this.step(1);
          break;
        case "Home":
          event.preventDefault();
          this.jump(0);
          break;
        case "End":
          event.preventDefault();
          this.jump(D.roundCount);
          break;
        case "+":
          this.shiftSpeed(1);
          break;
        case "-":
          this.shiftSpeed(-1);
          break;
        case "v":
        case "V":
          this.cycleView();
          break;
        default:
          break;
      }
    });
  },

  cycleView() {
    const order = ["enl", "res", "public", "god"].filter(
      (view) => view === "public" || view === "god" || Filter.embedded.includes(view)
    );
    const index = order.indexOf(this.state.view);
    this.setView(order[(index + 1) % order.length]);
  },

  shiftSpeed(direction) {
    const steps = [1000, 500, 250, 125];
    const index = steps.indexOf(this.state.speed);
    const next = Math.min(steps.length - 1, Math.max(0, index + direction));
    this.setSpeed(steps[next]);
    document.getElementById("speed").value = String(steps[next]);
  },

  setSpeed(ms) {
    this.state.speed = ms;
    if (this.state.playing) {
      this.stopTimer();
      this.startTimer();
    }
  },

  setView(view) {
    this.state.view = view;
    this.refresh();
  },

  togglePlay() {
    this.state.playing = !this.state.playing;
    if (this.state.playing) {
      if (this.state.round >= D.roundCount) this.jump(0);
      this.startTimer();
    } else {
      this.stopTimer();
    }
    this.refresh();
  },

  startTimer() {
    this.timer = window.setInterval(() => {
      if (this.state.round >= D.roundCount) {
        this.togglePlay();
        return;
      }
      this.jump(this.state.round + 1);
    }, this.state.speed);
  },

  stopTimer() {
    if (this.timer !== null) {
      window.clearInterval(this.timer);
      this.timer = null;
    }
  },

  step(delta) {
    this.jump(this.state.round + delta);
  },

  jump(round) {
    const clamped = Math.min(D.roundCount, Math.max(0, round));
    this.state.round = clamped;
    this.refresh();
  },

  refresh() {
    const { round, view, playing } = this.state;
    document.getElementById("slider").value = String(round);
    document.getElementById("round-readout").textContent =
      "round " + round + " / " + D.roundCount;
    document.getElementById("btn-play").textContent = playing ? "Pause" : "Play";
    document.querySelectorAll("#views button").forEach((button) => {
      button.classList.toggle("active", button.dataset.view === view);
    });
    Render.draw(round, view);
    Panels.updateCurve(round);
    Panels.updateEvents(round);
  },
};

// Boot (the exporter concatenates data/filter/render/panels/controls and
// then this file's sibling selftest.js runs after everything is live).
D.init();
Filter.init();
Render.init();
Panels.init();
Controls.init();
