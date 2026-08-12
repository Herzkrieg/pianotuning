/* ============ Particle background ============ */
const canvas = document.getElementById('particles');
const ctx = canvas.getContext('2d');
let particles = [];

function resize() {
  canvas.width = innerWidth;
  canvas.height = innerHeight;
}
addEventListener('resize', resize);
resize();

const COLORS = ['#69C9D0', '#EE1D52', '#ffffff'];
const COUNT = Math.min(90, Math.floor(innerWidth / 12));

for (let i = 0; i < COUNT; i++) {
  particles.push({
    x: Math.random() * canvas.width,
    y: Math.random() * canvas.height,
    r: Math.random() * 2.2 + 0.6,
    vx: (Math.random() - 0.5) * 0.4,
    vy: (Math.random() - 0.5) * 0.4,
    c: COLORS[Math.floor(Math.random() * COLORS.length)],
    a: Math.random() * 0.6 + 0.2
  });
}

(function animate() {
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  for (const p of particles) {
    p.x += p.vx; p.y += p.vy;
    if (p.x < 0) p.x = canvas.width; if (p.x > canvas.width) p.x = 0;
    if (p.y < 0) p.y = canvas.height; if (p.y > canvas.height) p.y = 0;
    ctx.globalAlpha = p.a;
    ctx.fillStyle = p.c;
    ctx.beginPath();
    ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.globalAlpha = 1;
  requestAnimationFrame(animate);
})();

/* ============ Audio engine (Web Audio API) ============ */
let audioCtx = null;

function getAudioCtx() {
  if (!audioCtx) {
    audioCtx = new (window.AudioContext || window.webkitAudioContext)();
  }
  if (audioCtx.state === 'suspended') {
    audioCtx.resume();
  }
  return audioCtx;
}

/* Play a single note with a piano-like envelope */
function playNote(freq, duration = 2, when = 0, gainLevel = 0.25) {
  const ac = getAudioCtx();
  const t = ac.currentTime + when;
  
  const master = ac.createGain();
  master.connect(ac.destination);
  
  // Layer a few harmonics for a piano-ish timbre
  const harmonics = [
    { mult: 1, gain: 1.0 },
    { mult: 2, gain: 0.35 },
    { mult: 3, gain: 0.15 },
    { mult: 4, gain: 0.07 }
  ];
  
  harmonics.forEach(h => {
    const osc = ac.createOscillator();
    const g = ac.createGain();
    osc.type = 'sine';
    osc.frequency.value = freq * h.mult;
    g.gain.value = 0;
    osc.connect(g);
    g.connect(master);
    
    // Attack + exponential decay (piano-like)
    g.gain.setValueAtTime(0, t);
    g.gain.linearRampToValueAtTime(gainLevel * h.gain, t + 0.01);
    g.gain.exponentialRampToValueAtTime(0.0001, t + duration);
    
    osc.start(t);
    osc.stop(t + duration + 0.1);
  });
}

/* ============ Pitch previews (432 / 440 / 444 / 528) ============ */
document.querySelectorAll('.play').forEach(btn => {
  btn.addEventListener('click', e => {
    e.stopPropagation(); // don't re-flip the card
    const freq = parseFloat(btn.dataset.freq);
    playNote(freq, 2.5);
  });
});

/* ============ Temperament previews (C major triad) ============ */
/* Ratios relative to C for C–E–G in each temperament */
const TEMPERAMENTS = {
  equal:       [1, Math.pow(2, 4 / 12), Math.pow(2, 7 / 12)], // 12-TET
  pythagorean: [1, 81 / 64, 3 / 2],                            // ditone third
  just:        [1, 5 / 4, 3 / 2],                              // pure intervals
  meantone:    [1, 5 / 4, Math.pow(5, 1 / 4)]                  // ¼-comma: pure third, narrowed fifth
};

const C4 = 261.63; // Middle C

document.querySelectorAll('.play-chord').forEach(btn => {
  btn.addEventListener('click', e => {
    e.stopPropagation();
    const ratios = TEMPERAMENTS[btn.dataset.temp] || TEMPERAMENTS.equal;
    // Arpeggiate slightly, then sustain together
    ratios.forEach((r, i) => playNote(C4 * r, 3, i * 0.12, 0.18));
  });
});

/* Flip cards: toggle on click, revert when clicking elsewhere */
document.querySelectorAll('.flip-card').forEach(card => {
  card.addEventListener('click', e => {
    e.stopPropagation();
    const wasFlipped = card.classList.contains('flipped');
    document.querySelectorAll('.flip-card.flipped')
    .forEach(c => c.classList.remove('flipped'));
    if (!wasFlipped) card.classList.add('flipped');
  });
});

document.addEventListener('click', () => {
  document.querySelectorAll('.flip-card.flipped')
  .forEach(c => c.classList.remove('flipped'));
});

/* ============ Piano nav — each key plays its own note @ A=432 ============ */
const A432 = 432;

const NOTE_OFFSETS = {
  'C4': -9, 'C#4': -8, 'D4': -7, 'D#4': -6, 'E4': -5, 'F4': -4,
  'F#4': -3, 'G4': -2, 'G#4': -1, 'A4': 0, 'A#4': 1, 'B4': 2
};

function noteFreq432(note) {
  return A432 * Math.pow(2, NOTE_OFFSETS[note] / 12);
}

const pianoKeys = document.querySelectorAll('.piano-key');

pianoKeys.forEach(key => {
  key.addEventListener('click', () => {
    const note = key.dataset.note;
    if (note && NOTE_OFFSETS[note] !== undefined) {
      playNote(noteFreq432(note), 1.8, 0, 0.2);
    }
    key.classList.add('pressed');
    setTimeout(() => key.classList.remove('pressed'), 200);
    
    const target = key.dataset.target && document.querySelector(key.dataset.target);
    if (target) target.scrollIntoView({ behavior: 'smooth' });
  });
});

const sections = document.querySelectorAll('.section');
const observer = new IntersectionObserver(entries => {
  entries.forEach(entry => {
    if (entry.isIntersecting) {
      pianoKeys.forEach(k => k.classList.toggle(
        'active',
        k.dataset.target === '#' + entry.target.id
      ));
    }
  });
}, { threshold: 0.5 });
sections.forEach(s => observer.observe(s));

/* ============ Email booking (Formspree) ============ */
const FORM_ENDPOINT = 'https://formspree.io/f/meajgelq'; // ← replace

document.getElementById('bookingForm').addEventListener('submit', async e => {
  e.preventDefault();
  const form = e.target;
  const status = document.getElementById('formStatus');
  const btn = form.querySelector('button[type="submit"]');
  
  btn.disabled = true;
  status.textContent = 'Sending…';
  
  try {
    const res = await fetch(FORM_ENDPOINT, {
      method: 'POST',
      body: new FormData(form),
      headers: { 'Accept': 'application/json' }
    });
    
    if (res.ok) {
      status.textContent = '✅ Booking request sent! We\'ll be in touch soon.';
      form.reset();
    } else {
      const data = await res.json();
      status.textContent = '⚠️ ' + (data.errors?.map(er => er.message).join(', ') || 'Something went wrong — please try again.');
    }
  } catch {
    status.textContent = '⚠️ Network error — please try again.';
  } finally {
    btn.disabled = false;
  }
});
