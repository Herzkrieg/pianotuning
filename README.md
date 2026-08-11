# 🎹 PerfectPitch Piano Care

A fun, interactive website for a local piano technician and tuning service.

## Features

- **Interactive piano keyboard** — play notes with your mouse or computer keyboard (A–K / W–U), plus one-click demo melodies
- **Piano tuning game** — hear a detuned string, adjust the slider, and try to tune it within 3 cents
- **Service catalog** — six services with one-click selection that pre-fills the booking form
- **Booking form** — validated appointment request form with confetti celebration
- **Testimonials carousel**, animated stats, FAQ accordion, floating musical notes, and scroll-reveal animations
- Fully responsive and respects `prefers-reduced-motion`

## Running locally

No build step needed — it's plain HTML/CSS/JS:

```bash
# from the repo root
python3 -m http.server 8000
# then open http://localhost:8000
```

Or simply open `index.html` in a browser. Sound requires a user interaction first (browser autoplay policy).
