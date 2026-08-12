# PianoTuner (Android, Rust)

A professional-grade piano tuning app written in Rust: FFT pitch detection,
per-string inharmonicity measurement, stretched (Railsback) tuning curves,
historical temperaments, pitch-raise overpull and a saveable tuning library.

The UI is [egui](https://github.com/emilk/egui)/eframe running on
`NativeActivity`, audio capture is [cpal](https://github.com/RustAudio/cpal)
(Oboe/AAudio backend), and all of the maths lives in a platform-independent
crate that is unit tested on the host.

## Layout

```
android/
├── Cargo.toml                  # workspace
├── crates/
│   ├── tuner-core/             # DSP + tuning logic (no Android deps)
│   │   └── src/
│   │       ├── cents.rs            # frequency <-> cents <-> key index
│   │       ├── inharmonicity.rs    # B coefficient fitting and interpolation
│   │       ├── pitch_detection.rs  # windowed FFT, peak + partial detection
│   │       ├── temperament.rs      # equal + 6 historical temperaments
│   │       ├── tuning_curve.rs     # stretch curve, targets, overpull
│   │       └── tuning_file.rs      # .ptun load/save/validate
│   └── app/                    # Android shell
│       └── src/
│           ├── lib.rs              # android_main / run_desktop entry points
│           ├── audio.rs            # cpal capture + ring buffer + analysis
│           ├── state.rs            # app state and user actions
│           ├── storage.rs          # tuning library on disk
│           └── ui.rs               # egui screens
├── docs/tuning-file-format.md
└── examples/*.ptun             # sample tuning files
```

## Building and testing on the host

Everything except microphone capture builds and runs on a normal desktop
toolchain, which is what CI checks:

```bash
cd android
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo test` covers the DSP (synthetic sine and inharmonic tones), the
inharmonicity fit, curve generation, temperament mapping, file validation,
storage sanitising and the app state machine.

## Building the APK

Requires the Android SDK + NDK and
[`cargo-apk`](https://crates.io/crates/cargo-apk):

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo install cargo-apk
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_NDK_ROOT=$ANDROID_HOME/ndk/27.3.13750724

cd android/crates/app
cargo apk build --lib            # debug APK in android/target/debug/apk/
cargo apk run --lib              # build, install and launch on a device
```

The manifest metadata in `crates/app/Cargo.toml` sets the package id
(`com.pianotuner.app`), `minSdkVersion` 24, `targetSdkVersion` 34 and requests
`RECORD_AUDIO` (plus legacy storage permissions for exporting tunings).
Android 6+ requires the microphone permission to be granted at runtime; grant it
from the app info screen or with
`adb shell pm grant com.pianotuner.app android.permission.RECORD_AUDIO`.

## Using the app

* **Tuner** – large note name, cents readout, colour-coded needle and a strobe
  that stands still when the note is in tune. The 88-key strip shows the
  detected note; tap a key to lock the tuner to it (useful for very quiet or
  ambiguous bass notes). Measured notes are highlighted in blue.
* **Spectrum** – live magnitude spectrum with the detected partials marked and
  their deviation from a pure harmonic series.
* **Measure** – play a note, then store its inharmonicity coefficient `B`. The
  app fits `f_n = n·f1·√(1 + B·n²)` to the detected partials, log-interpolates a
  `B` value for all 88 keys and regenerates the tuning curve, which is plotted
  underneath.
* **Settings** – A4 reference (415–466 Hz), device calibration, stretch amount,
  per-interval weights, temperament selection and pitch-raise overpull.
* **Library** – name, save, reload and delete tunings, or copy the current
  tuning to the clipboard as JSON.

## Tuning theory in one paragraph

Real strings are stiff, so their partials are sharp: `f_n = n·f1·√(1 + B·n²)`.
Because tuners match partials rather than fundamentals, a piano tuned to beatless
octaves ends up *stretched* — flat in the bass and sharp in the treble (the
Railsback curve). `tuner-core` measures `B` from the spectrum of a played note,
interpolates it across the keyboard and derives the cent offset for each key,
weighted by how much the tuner cares about octaves, fifths, twelfths and double
octaves. Historical temperaments add a further per-pitch-class offset, and pitch
raise mode adds overpull so that a heavily flat piano settles near pitch.

## File format

Tunings are JSON files with a `.ptun` extension; see
[docs/tuning-file-format.md](docs/tuning-file-format.md) and the ready-made
examples in [`examples/`](examples).

## CI

[`.github/workflows/android.yml`](../.github/workflows/android.yml) runs
formatting, clippy and tests on the host, then cross-compiles the Android
targets and builds a debug APK which is uploaded as an artifact.
