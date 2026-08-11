/* ================================================================
   PerfectPitch Piano Care — interactive behaviors
   ================================================================ */
(function () {
  "use strict";

  /* ---------- Audio (Web Audio API) ---------- */
  let audioCtx = null;
  function getCtx() {
    if (!audioCtx) {
      audioCtx = new (window.AudioContext || window.webkitAudioContext)();
    }
    if (audioCtx.state === "suspended") audioCtx.resume();
    return audioCtx;
  }

  // Simple piano-ish tone: two detuned triangles + fast decay envelope
  function playNote(freq, duration = 1.1, when = 0, gainLevel = 0.22) {
    const ctx = getCtx();
    const t0 = ctx.currentTime + when;
    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.0001, t0);
    gain.gain.exponentialRampToValueAtTime(gainLevel, t0 + 0.015);
    gain.gain.exponentialRampToValueAtTime(0.0001, t0 + duration);
    gain.connect(ctx.destination);

    [0, 2.5].forEach(function (detune) {
      const osc = ctx.createOscillator();
      osc.type = "triangle";
      osc.frequency.setValueAtTime(freq, t0);
      osc.detune.setValueAtTime(detune, t0);
      osc.connect(gain);
      osc.start(t0);
      osc.stop(t0 + duration + 0.05);
    });
  }

  /* ---------- Interactive piano ---------- */
  const NOTES = [
    { name: "C4", freq: 261.63, type: "white", key: "a" },
    { name: "C#4", freq: 277.18, type: "black", key: "w" },
    { name: "D4", freq: 293.66, type: "white", key: "s" },
    { name: "D#4", freq: 311.13, type: "black", key: "e" },
    { name: "E4", freq: 329.63, type: "white", key: "d" },
    { name: "F4", freq: 349.23, type: "white", key: "f" },
    { name: "F#4", freq: 369.99, type: "black", key: "t" },
    { name: "G4", freq: 392.0, type: "white", key: "g" },
    { name: "G#4", freq: 415.3, type: "black", key: "y" },
    { name: "A4", freq: 440.0, type: "white", key: "h" },
    { name: "A#4", freq: 466.16, type: "black", key: "u" },
    { name: "B4", freq: 493.88, type: "white", key: "j" },
    { name: "C5", freq: 523.25, type: "white", key: "k" },
  ];

  const piano = document.getElementById("pianoKeys");
  const keyEls = {};
  NOTES.forEach(function (note) {
    const el = document.createElement("button");
    el.className = "key " + note.type;
    el.type = "button";
    el.dataset.note = note.name;
    el.setAttribute("aria-label", "Piano key " + note.name);
    el.innerHTML = '<span class="key-label">' + note.key.toUpperCase() + "</span>";
    el.addEventListener("pointerdown", function () { strike(note); });
    piano.appendChild(el);
    keyEls[note.name] = el;
  });

  function strike(note) {
    playNote(note.freq);
    const el = keyEls[note.name];
    el.classList.add("active");
    setTimeout(function () { el.classList.remove("active"); }, 180);
  }

  // Computer keyboard support
  const keyMap = {};
  NOTES.forEach(function (n) { keyMap[n.key] = n; });
  document.addEventListener("keydown", function (e) {
    if (e.repeat) return;
    const tag = (e.target.tagName || "").toLowerCase();
    if (tag === "input" || tag === "textarea" || tag === "select") return;
    const note = keyMap[e.key.toLowerCase()];
    if (note) strike(note);
  });

  /* ---------- Demo melodies ---------- */
  const MELODIES = {
    twinkle: [
      ["C4", 0.4], ["C4", 0.4], ["G4", 0.4], ["G4", 0.4],
      ["A4", 0.4], ["A4", 0.4], ["G4", 0.8],
      ["F4", 0.4], ["F4", 0.4], ["E4", 0.4], ["E4", 0.4],
      ["D4", 0.4], ["D4", 0.4], ["C4", 0.8],
    ],
    ode: [
      ["E4", 0.4], ["E4", 0.4], ["F4", 0.4], ["G4", 0.4],
      ["G4", 0.4], ["F4", 0.4], ["E4", 0.4], ["D4", 0.4],
      ["C4", 0.4], ["C4", 0.4], ["D4", 0.4], ["E4", 0.4],
      ["E4", 0.6], ["D4", 0.2], ["D4", 0.8],
    ],
    fur: [
      ["E4", 0.3], ["D#4", 0.3], ["E4", 0.3], ["D#4", 0.3],
      ["E4", 0.3], ["B4", 0.3], ["D4", 0.3], ["C4", 0.3],
      ["A4", 0.9],
    ],
  };

  let melodyTimer = null;
  document.querySelectorAll(".play-melody").forEach(function (btn) {
    btn.addEventListener("click", function () {
      if (melodyTimer) { clearTimeout(melodyTimer); melodyTimer = null; }
      document.querySelectorAll(".play-melody").forEach(function (b) { b.classList.remove("playing"); });
      btn.classList.add("playing");
      const seq = MELODIES[btn.dataset.melody];
      const noteByName = {};
      NOTES.forEach(function (n) { noteByName[n.name] = n; });
      let i = 0;
      (function step() {
        if (i >= seq.length) { btn.classList.remove("playing"); melodyTimer = null; return; }
        const pair = seq[i++];
        strike(noteByName[pair[0]]);
        melodyTimer = setTimeout(step, pair[1] * 1000);
      })();
    });
  });

  /* ---------- Tuning game ---------- */
  const slider = document.getElementById("tunerSlider");
  const needle = document.getElementById("gaugeNeedle");
  const readout = document.getElementById("centsReadout");
  const status = document.getElementById("tunerStatus");
  const scoreEl = document.getElementById("tunerScore");
  const BASE_FREQ = 440;
  let detuneOffset = randomOffset(); // cents the string is out of tune
  let score = 0;
  let revealed = false;

  function randomOffset() {
    let off = 0;
    while (Math.abs(off) < 12) off = Math.round((Math.random() * 90 - 45) * 2) / 2;
    return off;
  }
  function centsToFreq(base, cents) { return base * Math.pow(2, cents / 1200); }
  function currentCents() { return detuneOffset + parseFloat(slider.value); }

  function updateNeedle() {
    if (!revealed) return;
    const cents = Math.max(-50, Math.min(50, currentCents()));
    needle.style.left = (50 + cents) + "%";
    readout.textContent = (cents > 0 ? "+" : "") + cents.toFixed(1);
    const abs = Math.abs(currentCents());
    if (abs <= 3) {
      status.textContent = "🎯 Sounds perfect — hit Check Tuning!";
      status.className = "tuner-status win";
    } else if (abs <= 10) {
      status.textContent = "Getting close… listen for the beats slowing down.";
      status.className = "tuner-status close";
    } else {
      status.textContent = "Still out of tune. Keep adjusting!";
      status.className = "tuner-status";
    }
  }

  document.getElementById("tunerPlay").addEventListener("click", function () {
    revealed = true;
    // Reference tone + detuned "string" together: you hear beats
    playNote(BASE_FREQ, 2.2, 0, 0.15);
    playNote(centsToFreq(BASE_FREQ, currentCents()), 2.2, 0, 0.15);
    updateNeedle();
  });

  slider.addEventListener("input", function () {
    if (!revealed) return;
    updateNeedle();
    playNote(centsToFreq(BASE_FREQ, currentCents()), 0.35, 0, 0.12);
  });

  document.getElementById("tunerCheck").addEventListener("click", function () {
    if (!revealed) { showToast("Press ▶ Play Tone first to hear the string!"); return; }
    const abs = Math.abs(currentCents());
    if (abs <= 3) {
      score++;
      scoreEl.textContent = score;
      status.textContent = "🏆 Perfectly tuned! You have a technician's ear!";
      status.className = "tuner-status win";
      confettiBurst();
      playNote(BASE_FREQ, 1.6, 0, 0.2);
      playNote(BASE_FREQ * 1.25, 1.6, 0.12, 0.15);
      playNote(BASE_FREQ * 1.5, 1.6, 0.24, 0.15);
    } else {
      status.textContent = "Off by " + abs.toFixed(1) + " cents — real tunings take practice. Try again!";
      status.className = "tuner-status close";
    }
  });

  document.getElementById("tunerNew").addEventListener("click", function () {
    detuneOffset = randomOffset();
    slider.value = 0;
    revealed = false;
    needle.style.left = "50%";
    readout.textContent = "?";
    status.textContent = "Press ▶ to hear the out-of-tune string";
    status.className = "tuner-status";
  });

  /* ---------- Navbar ---------- */
  const navbar = document.getElementById("navbar");
  const navToggle = document.getElementById("navToggle");
  const navLinks = document.getElementById("navLinks");
  window.addEventListener("scroll", function () {
    navbar.classList.toggle("scrolled", window.scrollY > 10);
  }, { passive: true });
  navToggle.addEventListener("click", function () {
    const open = navLinks.classList.toggle("open");
    navToggle.setAttribute("aria-expanded", open ? "true" : "false");
  });
  navLinks.addEventListener("click", function (e) {
    if (e.target.tagName === "A") navLinks.classList.remove("open");
  });

  /* ---------- Scroll reveal ---------- */
  const observer = new IntersectionObserver(function (entries) {
    entries.forEach(function (entry) {
      if (entry.isIntersecting) {
        entry.target.classList.add("visible");
        observer.unobserve(entry.target);
      }
    });
  }, { threshold: 0.12 });
  document.querySelectorAll(".reveal").forEach(function (el) { observer.observe(el); });

  /* ---------- Animated counters ---------- */
  const counterObserver = new IntersectionObserver(function (entries) {
    entries.forEach(function (entry) {
      if (!entry.isIntersecting) return;
      const el = entry.target;
      counterObserver.unobserve(el);
      const target = parseInt(el.dataset.count, 10);
      const dur = 1400;
      let start = null;
      requestAnimationFrame(function tick(now) {
        if (start === null) start = now;
        const p = Math.min((now - start) / dur, 1);
        el.textContent = Math.round(target * (1 - Math.pow(1 - p, 3))).toLocaleString();
        if (p < 1) requestAnimationFrame(tick);
      });
    });
  }, { threshold: 0.5 });
  document.querySelectorAll(".stat-num").forEach(function (el) { counterObserver.observe(el); });

  /* ---------- Floating notes in hero ---------- */
  const notesWrap = document.querySelector(".hero-notes");
  const glyphs = ["♪", "♫", "♩", "♬", "𝄞"];
  for (let i = 0; i < 14; i++) {
    const n = document.createElement("span");
    n.className = "note";
    n.textContent = glyphs[i % glyphs.length];
    n.style.left = Math.random() * 100 + "%";
    n.style.animationDuration = 9 + Math.random() * 10 + "s";
    n.style.animationDelay = Math.random() * 12 + "s";
    n.style.fontSize = 1 + Math.random() * 1.4 + "rem";
    notesWrap.appendChild(n);
  }

  /* ---------- Service cards → booking form ---------- */
  document.querySelectorAll(".service-card").forEach(function (card) {
    card.querySelector(".select-service").addEventListener("click", function () {
      document.querySelectorAll(".service-card").forEach(function (c) { c.classList.remove("selected"); });
      card.classList.add("selected");
      const select = document.getElementById("service");
      const name = card.dataset.service;
      Array.from(select.options).forEach(function (opt) {
        if (opt.value === name) select.value = opt.value;
      });
      showToast("✅ " + name + " selected — finish booking below!");
      document.getElementById("booking").scrollIntoView({ behavior: "smooth" });
    });
  });

  /* ---------- Testimonials carousel ---------- */
  const slides = Array.from(document.querySelectorAll(".testimonial"));
  const dotsWrap = document.getElementById("carouselDots");
  let slideIndex = 0;
  slides.forEach(function (_, i) {
    const dot = document.createElement("button");
    dot.setAttribute("aria-label", "Show review " + (i + 1));
    dot.addEventListener("click", function () { goTo(i); });
    dotsWrap.appendChild(dot);
  });
  const dots = Array.from(dotsWrap.children);
  function goTo(i) {
    slideIndex = (i + slides.length) % slides.length;
    slides.forEach(function (s, j) { s.classList.toggle("active", j === slideIndex); });
    dots.forEach(function (d, j) { d.classList.toggle("active", j === slideIndex); });
  }
  document.getElementById("carouselPrev").addEventListener("click", function () { goTo(slideIndex - 1); resetAuto(); });
  document.getElementById("carouselNext").addEventListener("click", function () { goTo(slideIndex + 1); resetAuto(); });
  let autoTimer = setInterval(function () { goTo(slideIndex + 1); }, 6000);
  function resetAuto() { clearInterval(autoTimer); autoTimer = setInterval(function () { goTo(slideIndex + 1); }, 6000); }
  goTo(0);

  /* ---------- Booking form ---------- */
  const form = document.getElementById("bookingForm");
  const dateInput = document.getElementById("date");
  dateInput.min = new Date().toISOString().split("T")[0];

  form.addEventListener("submit", function (e) {
    e.preventDefault();
    let valid = true;
    form.querySelectorAll("[required]").forEach(function (field) {
      const ok = field.checkValidity();
      field.classList.toggle("invalid", !ok);
      if (!ok) valid = false;
    });
    if (!valid) {
      showToast("⚠️ Please fill in the highlighted fields.");
      return;
    }
    const name = document.getElementById("name").value.trim().split(" ")[0];
    form.reset();
    form.querySelectorAll(".invalid").forEach(function (f) { f.classList.remove("invalid"); });
    confettiBurst();
    showToast("🎉 Thanks " + name + "! We'll confirm your appointment within 24 hours.");
  });

  form.querySelectorAll("[required]").forEach(function (field) {
    field.addEventListener("input", function () { field.classList.remove("invalid"); });
  });

  /* ---------- Toast ---------- */
  const toast = document.getElementById("toast");
  let toastTimer = null;
  function showToast(msg) {
    toast.textContent = msg;
    toast.classList.add("show");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(function () { toast.classList.remove("show"); }, 4200);
  }

  /* ---------- Confetti ---------- */
  function confettiBurst() {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const colors = ["#b8860b", "#d4a017", "#f3e3bd", "#2e2a24", "#a9cf9f"];
    for (let i = 0; i < 60; i++) {
      const c = document.createElement("div");
      c.className = "confetti";
      c.style.left = Math.random() * 100 + "vw";
      c.style.background = colors[i % colors.length];
      c.style.animationDuration = 1.6 + Math.random() * 1.6 + "s";
      c.style.animationDelay = Math.random() * 0.4 + "s";
      c.style.transform = "rotate(" + Math.random() * 360 + "deg)";
      document.body.appendChild(c);
      setTimeout(function () { c.remove(); }, 4000);
    }
  }

  /* ---------- Footer year ---------- */
  document.getElementById("year").textContent = new Date().getFullYear();
})();
