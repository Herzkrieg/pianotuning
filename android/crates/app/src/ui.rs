//! egui user interface: tuner, spectrum, measurement, settings and library screens.

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Stroke, Vec2};

use tuner_core::{
    cents::{key_name, key_to_freq},
    NUM_KEYS,
};

use crate::audio::AudioEngine;
use crate::state::{AppState, Screen, CLOSE_CENTS, IN_TUNE_CENTS};

/// Full scale deflection of the needle meter, in cents.
const METER_RANGE_CENTS: f32 = 50.0;

/// The eframe application.
pub struct PianoTunerApp {
    pub state: AppState,
    audio: AudioEngine,
    strobe_phase: f64,
    last_update: std::time::Instant,
}

impl PianoTunerApp {
    /// Build the app, load the on-disk tuning library and start audio capture.
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut state = AppState::default();
        state.update_tuning_curve();
        state.refresh_library();

        let audio = AudioEngine::new();
        if let Ok(audio_state) = audio.state.lock() {
            if let Some(err) = &audio_state.error {
                state.error_message = err.clone();
            }
        }

        Self {
            state,
            audio,
            strobe_phase: 0.0,
            last_update: std::time::Instant::now(),
        }
    }

    /// Pull the newest analysis result out of the audio engine.
    fn update_audio_state(&mut self) {
        let dt = self.last_update.elapsed().as_secs_f64();
        self.last_update = std::time::Instant::now();

        let latest = match self.audio.state.lock() {
            Ok(audio_state) => audio_state.latest_result.clone(),
            Err(_) => {
                self.state.error_message = "Audio state unavailable".into();
                return;
            }
        };

        let Some(result) = latest else { return };

        self.state.apply_detection(
            result.fundamental_hz,
            result.partials,
            result.magnitude_spectrum,
            result.bin_hz,
        );

        // The strobe rotates at a rate proportional to the pitch error, so it
        // stands still when the note is in tune.
        let cents = self.state.detected_cents_deviation;
        self.strobe_phase = (self.strobe_phase + cents * dt * 20.0).rem_euclid(360.0);
    }

    fn deviation_color(cents: f64) -> Color32 {
        if cents.abs() < IN_TUNE_CENTS {
            Color32::GREEN
        } else if cents.abs() < CLOSE_CENTS {
            Color32::YELLOW
        } else {
            Color32::RED
        }
    }
}

impl eframe::App for PianoTunerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_audio_state();
        ctx.request_repaint(); // continuous animation

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (screen, label) in [
                    (Screen::Tuner, "Tuner"),
                    (Screen::Spectrum, "Spectrum"),
                    (Screen::Measurement, "Measure"),
                    (Screen::Settings, "Settings"),
                    (Screen::Library, "Library"),
                ] {
                    if ui
                        .selectable_label(self.state.current_screen == screen, label)
                        .clicked()
                    {
                        self.state.current_screen = screen;
                    }
                }
            });
        });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            if !self.state.error_message.is_empty() {
                let msg = self.state.error_message.clone();
                ui.colored_label(Color32::LIGHT_RED, msg);
            } else if !self.state.status_message.is_empty() {
                let msg = self.state.status_message.clone();
                ui.colored_label(Color32::LIGHT_GREEN, msg);
            } else {
                ui.label(format!(
                    "A4 = {:.1} Hz  |  {}  |  stretch {:.2}",
                    self.state.a4_hz,
                    self.state.temperament().name,
                    self.state.stretch
                ));
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.state.current_screen {
                Screen::Tuner => self.draw_tuner_screen(ui),
                Screen::Spectrum => self.draw_spectrum_screen(ui),
                Screen::Measurement => self.draw_measurement_screen(ui),
                Screen::Settings => self.draw_settings_screen(ui),
                Screen::Library => self.draw_library_screen(ui),
            });
        });
    }
}

impl PianoTunerApp {
    fn draw_tuner_screen(&mut self, ui: &mut egui::Ui) {
        let key_idx = self.state.active_key().unwrap_or(tuner_core::A4_KEY_INDEX);
        let cents = self.state.detected_cents_deviation;
        let cents_color = Self::deviation_color(cents);

        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(key_name(key_idx)).size(64.0).strong());

            match self.state.detected_fundamental_hz {
                Some(hz) => ui.label(format!(
                    "{hz:.2} Hz  ->  target {:.2} Hz",
                    self.state.target_freq(key_idx)
                )),
                None => ui.label("Listening..."),
            };

            ui.label(
                egui::RichText::new(format!("{cents:+.1} cents"))
                    .size(32.0)
                    .color(cents_color),
            );

            if self.state.detected_fundamental_hz.is_some() && cents.abs() < IN_TUNE_CENTS {
                ui.label(
                    egui::RichText::new("IN TUNE")
                        .size(24.0)
                        .color(Color32::GREEN)
                        .strong(),
                );
            } else {
                ui.label(if cents < 0.0 { "flat" } else { "sharp" });
            }

            ui.add_space(10.0);
            self.draw_strobe(ui);
            ui.add_space(10.0);
            draw_needle(ui, cents, cents_color);
            ui.add_space(10.0);

            let lock_label = match self.state.locked_key_index {
                Some(k) => format!("Unlock {}", key_name(k)),
                None => "Lock to detected note".to_string(),
            };
            if ui.button(lock_label).clicked() {
                self.state.locked_key_index = match self.state.locked_key_index {
                    Some(_) => None,
                    None => self.state.detected_key_index,
                };
            }
        });

        ui.add_space(10.0);
        self.draw_keyboard_strip(ui);
    }

    fn draw_strobe(&self, ui: &mut egui::Ui) {
        let (response, painter) =
            ui.allocate_painter(Vec2::new(200.0, 200.0), egui::Sense::hover());
        let center = response.rect.center();
        let radius = (response.rect.width().min(response.rect.height()) / 2.0 - 10.0).max(10.0);

        let phase_rad = (self.strobe_phase as f32).to_radians();
        let n_bars = 8;
        for i in 0..n_bars {
            let angle = phase_rad + (i as f32) * std::f32::consts::TAU / n_bars as f32;
            let (sin, cos) = angle.sin_cos();
            let inner = Pos2::new(center.x + radius * 0.5 * cos, center.y + radius * 0.5 * sin);
            let outer = Pos2::new(center.x + radius * cos, center.y + radius * sin);
            let color = if i % 2 == 0 {
                Color32::WHITE
            } else {
                Color32::DARK_GRAY
            };
            painter.line_segment([inner, outer], Stroke::new(4.0_f32, color));
        }
        painter.circle_stroke(center, radius, Stroke::new(2.0_f32, Color32::GRAY));
    }

    fn draw_keyboard_strip(&mut self, ui: &mut egui::Ui) {
        ui.label("Tap a key to lock the tuner to it:");
        let active = self.state.active_key();
        let available = ui.available_width().max(88.0);
        let key_w = (available / NUM_KEYS as f32).clamp(3.0, 12.0);
        let key_h = 36.0;

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for k in 0..NUM_KEYS {
                let is_sharp = key_name(k).contains('#');
                let is_active = active == Some(k);
                let is_measured = self.state.measured_b_values.contains_key(&k);
                let color = if is_active {
                    Color32::YELLOW
                } else if is_measured {
                    Color32::from_rgb(120, 180, 255)
                } else if is_sharp {
                    Color32::from_gray(40)
                } else {
                    Color32::WHITE
                };
                let (response, painter) =
                    ui.allocate_painter(Vec2::new(key_w, key_h), egui::Sense::click());
                let rect = response.rect;
                painter.rect_filled(rect, Rounding::ZERO, color);
                painter.rect_stroke(rect, Rounding::ZERO, Stroke::new(0.5_f32, Color32::BLACK));
                let response = response.on_hover_text(key_name(k));
                if response.clicked() {
                    self.state.locked_key_index = if self.state.locked_key_index == Some(k) {
                        None
                    } else {
                        Some(k)
                    };
                }
            }
        });
    }

    fn draw_spectrum_screen(&mut self, ui: &mut egui::Ui) {
        ui.heading("Spectrum");
        let bin_hz = self.state.spectrum_bin_hz;
        if self.state.magnitude_spectrum.is_empty() {
            ui.label("No audio analysed yet.");
            return;
        }
        ui.label(format!("Resolution: {bin_hz:.2} Hz per bin"));

        let width = ui.available_width().max(100.0);
        let (response, painter) =
            ui.allocate_painter(Vec2::new(width, 260.0), egui::Sense::hover());
        let rect = response.rect;
        painter.rect_filled(rect, Rounding::ZERO, Color32::BLACK);

        let spec = &self.state.magnitude_spectrum;
        let max_mag = spec.iter().copied().fold(0.0_f32, f32::max).max(1e-6);
        // Only draw up to the top of the piano range plus some headroom.
        let max_bin = if bin_hz > 0.0 {
            ((5000.0 / bin_hz) as usize).clamp(16, spec.len())
        } else {
            spec.len()
        };
        let n = max_bin.min(spec.len());
        let w = rect.width() / n as f32;

        for (i, &mag) in spec[..n].iter().enumerate() {
            let h = (mag / max_mag).sqrt() * rect.height();
            let x = rect.left() + i as f32 * w;
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(x, rect.bottom() - h),
                    Pos2::new(x + w.max(1.0), rect.bottom()),
                ),
                Rounding::ZERO,
                Color32::LIGHT_BLUE,
            );
        }

        if bin_hz > 0.0 {
            for &(partial, freq) in &self.state.detected_partials {
                let bin = (freq / bin_hz) as usize;
                if bin < n {
                    let x = rect.left() + bin as f32 * w;
                    painter.line_segment(
                        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                        Stroke::new(1.0_f32, Color32::RED),
                    );
                    painter.text(
                        Pos2::new(x + 2.0, rect.top() + 2.0),
                        egui::Align2::LEFT_TOP,
                        format!("{partial}"),
                        egui::FontId::proportional(10.0),
                        Color32::LIGHT_RED,
                    );
                }
            }
        }

        ui.add_space(8.0);
        ui.label("Detected partials:");
        for &(n, f) in &self.state.detected_partials {
            let ideal = self
                .state
                .detected_fundamental_hz
                .map(|f1| f1 * n as f64)
                .unwrap_or(f);
            ui.label(format!(
                "  {n}: {f:.2} Hz ({:+.1} cents from harmonic)",
                tuner_core::cents::freq_to_cents(f, ideal)
            ));
        }
    }

    fn draw_measurement_screen(&mut self, ui: &mut egui::Ui) {
        ui.heading("Inharmonicity Measurements");
        ui.label("Play a note firmly, then store its inharmonicity coefficient B.");

        if ui.button("Store B for current note").clicked() {
            match self.state.store_current_measurement() {
                Ok((key, b)) => {
                    self.state.status_message = format!("Stored B = {b:.6} for {}", key_name(key));
                    self.state.error_message.clear();
                }
                Err(e) => self.state.error_message = e,
            }
        }

        ui.add_space(5.0);
        ui.label("Measured B values:");
        let mut keys: Vec<usize> = self.state.measured_b_values.keys().copied().collect();
        keys.sort_unstable();
        let mut to_clear = None;
        for k in keys {
            let b = self.state.measured_b_values.get(&k).copied().unwrap_or(0.0);
            ui.horizontal(|ui| {
                ui.label(format!("{}: B = {b:.6}", key_name(k)));
                if ui.small_button("Clear").clicked() {
                    to_clear = Some(k);
                }
            });
        }
        if let Some(k) = to_clear {
            self.state.clear_measurement(k);
        }
        if self.state.measured_b_values.is_empty() {
            ui.label("  (none yet - a default curve is being used)");
        }

        ui.add_space(10.0);
        ui.heading("Tuning Curve");
        ui.label("Cent deviation from equal temperament, A0 (left) to C8 (right).");
        draw_curve(ui, &self.state.tuning_curve);

        ui.add_space(10.0);
        if ui.button("Recompute curve").clicked() {
            self.state.update_tuning_curve();
            self.state.status_message = "Tuning curve recomputed".into();
        }
    }

    fn draw_settings_screen(&mut self, ui: &mut egui::Ui) {
        ui.heading("Settings");

        let mut curve_dirty = false;

        ui.horizontal(|ui| {
            ui.label("A4 reference (Hz):");
            let mut a4 = self.state.a4_hz;
            if ui
                .add(egui::Slider::new(&mut a4, 415.0..=466.0).fixed_decimals(1))
                .changed()
            {
                self.state.a4_hz = a4;
                curve_dirty = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Calibration (cents):");
            let mut cal = self.state.calibration_cents;
            if ui
                .add(egui::Slider::new(&mut cal, -50.0..=50.0).fixed_decimals(1))
                .changed()
            {
                self.state.calibration_cents = cal;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Stretch:");
            let mut stretch = self.state.stretch;
            if ui
                .add(egui::Slider::new(&mut stretch, 0.0..=3.0).fixed_decimals(2))
                .changed()
            {
                self.state.stretch = stretch;
                curve_dirty = true;
            }
        });

        ui.add_space(6.0);
        ui.label("Interval weights");
        for (label, getter) in [
            ("Octave", 0usize),
            ("Fifth", 1),
            ("Twelfth", 2),
            ("Double octave", 3),
        ] {
            ui.horizontal(|ui| {
                ui.label(format!("{label}:"));
                let weights = &mut self.state.interval_weights;
                let value = match getter {
                    0 => &mut weights.octave,
                    1 => &mut weights.fifth,
                    2 => &mut weights.twelfth,
                    _ => &mut weights.double_octave,
                };
                if ui
                    .add(egui::Slider::new(value, 0.0..=2.0).fixed_decimals(2))
                    .changed()
                {
                    curve_dirty = true;
                }
            });
        }

        ui.add_space(10.0);
        ui.heading("Temperament");
        let mut selected = self.state.temperament_index;
        for (i, t) in self.state.temperaments.iter().enumerate() {
            if ui.radio(selected == i, &t.name).clicked() {
                selected = i;
            }
        }
        self.state.temperament_index = selected;

        ui.add_space(10.0);
        ui.heading("Pitch Raise");
        ui.checkbox(&mut self.state.pitch_raise_mode, "Enable pitch raise mode");
        if self.state.pitch_raise_mode {
            ui.horizontal(|ui| {
                ui.label("Overpull factor:");
                let mut f = self.state.overpull_factor;
                if ui
                    .add(egui::Slider::new(&mut f, 0.0..=1.0).fixed_decimals(2))
                    .changed()
                {
                    self.state.overpull_factor = f;
                }
            });
            if ui.button("Reset measured deviations").clicked() {
                self.state.current_deviation = vec![0.0; NUM_KEYS];
                self.state.status_message = "Pitch raise deviations cleared".into();
            }
        }

        ui.add_space(10.0);
        ui.separator();
        ui.label(format!(
            "A0 target {:.2} Hz  |  C8 target {:.2} Hz",
            self.state.target_freq(0),
            self.state.target_freq(NUM_KEYS - 1)
        ));
        ui.label(format!(
            "Equal tempered C8 would be {:.2} Hz",
            key_to_freq(NUM_KEYS - 1, self.state.a4_hz)
        ));

        if curve_dirty {
            self.state.update_tuning_curve();
        }
    }

    fn draw_library_screen(&mut self, ui: &mut egui::Ui) {
        ui.heading("Tuning Library");

        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.state.current_tuning_name);
        });

        ui.horizontal(|ui| {
            if ui.button("Save current tuning").clicked() {
                self.state.save_current_tuning();
            }
            if ui.button("Refresh").clicked() {
                self.state.refresh_library();
                self.state.status_message = "Library reloaded".into();
            }
        });

        ui.add_space(10.0);
        ui.label(format!(
            "Saved tunings in {}:",
            crate::storage::library_dir().display()
        ));

        let mut to_load: Option<usize> = None;
        let mut to_delete: Option<usize> = None;
        for (i, t) in self.state.saved_tunings.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{} (A4 {:.1} Hz)", t.name, t.a4_reference_hz));
                if ui.small_button("Load").clicked() {
                    to_load = Some(i);
                }
                if ui.small_button("Delete").clicked() {
                    to_delete = Some(i);
                }
            });
        }
        if self.state.saved_tunings.is_empty() {
            ui.label("  (none saved yet)");
        }

        if let Some(i) = to_load {
            if let Some(file) = self.state.saved_tunings.get(i).cloned() {
                let name = file.name.clone();
                self.state.import_tuning_file(file);
                self.state.status_message = format!("Loaded {name}");
                self.state.error_message.clear();
            }
        }
        if let Some(i) = to_delete {
            self.state.delete_tuning(i);
        }

        ui.add_space(10.0);
        ui.separator();
        ui.heading("Export / Import JSON");
        if ui.button("Copy current tuning as JSON").clicked() {
            match self.state.export_tuning_file().to_json() {
                Ok(json) => {
                    ui.output_mut(|o| o.copied_text = json);
                    self.state.status_message = "JSON copied to clipboard".into();
                    self.state.error_message.clear();
                }
                Err(e) => self.state.error_message = e,
            }
        }
    }
}

/// Draw the ±50 cent needle meter.
fn draw_needle(ui: &mut egui::Ui, cents: f64, color: Color32) {
    let width = ui.available_width().clamp(120.0, 320.0);
    let (response, painter) = ui.allocate_painter(Vec2::new(width, 60.0), egui::Sense::hover());
    let rect = response.rect;
    let mid_x = rect.center().x;

    painter.rect_filled(rect, Rounding::same(4.0), Color32::from_gray(40));
    painter.line_segment(
        [
            Pos2::new(mid_x, rect.top()),
            Pos2::new(mid_x, rect.bottom()),
        ],
        Stroke::new(1.0_f32, Color32::GRAY),
    );

    // Tick marks every 10 cents.
    for tick in (-40..=40).step_by(10) {
        let x = mid_x + (tick as f32 / METER_RANGE_CENTS) * (rect.width() / 2.0);
        painter.line_segment(
            [
                Pos2::new(x, rect.bottom() - 8.0),
                Pos2::new(x, rect.bottom()),
            ],
            Stroke::new(1.0_f32, Color32::from_gray(90)),
        );
    }

    let needle_x = (mid_x + (cents as f32 / METER_RANGE_CENTS) * (rect.width() / 2.0))
        .clamp(rect.left(), rect.right());
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(needle_x - 3.0, rect.top() + 5.0),
            Pos2::new(needle_x + 3.0, rect.bottom() - 5.0),
        ),
        Rounding::same(2.0),
        color,
    );
}

/// Draw an 88 point cent-offset curve.
fn draw_curve(ui: &mut egui::Ui, curve: &[f64]) {
    let width = ui.available_width().max(100.0);
    let (response, painter) = ui.allocate_painter(Vec2::new(width, 160.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, Rounding::ZERO, Color32::from_gray(20));
    painter.line_segment(
        [
            Pos2::new(rect.left(), rect.center().y),
            Pos2::new(rect.right(), rect.center().y),
        ],
        Stroke::new(1.0_f32, Color32::GRAY),
    );

    if curve.is_empty() {
        return;
    }

    let max_cents = curve
        .iter()
        .fold(1.0_f64, |acc, v| acc.max(v.abs()))
        .min(50.0) as f32;
    let w = rect.width() / curve.len() as f32;
    let mut previous: Option<Pos2> = None;

    for (i, &cents) in curve.iter().enumerate() {
        let normalized = (cents as f32 / max_cents).clamp(-1.0, 1.0);
        let point = Pos2::new(
            rect.left() + i as f32 * w,
            rect.center().y - normalized * rect.height() * 0.4,
        );
        if let Some(prev) = previous {
            painter.line_segment([prev, point], Stroke::new(1.5_f32, Color32::YELLOW));
        }
        painter.circle_filled(point, 1.5, Color32::YELLOW);
        previous = Some(point);
    }

    painter.text(
        Pos2::new(rect.left() + 4.0, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        format!("+/-{max_cents:.1} cents"),
        egui::FontId::proportional(11.0),
        Color32::GRAY,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deviation_colors() {
        assert_eq!(PianoTunerApp::deviation_color(0.0), Color32::GREEN);
        assert_eq!(PianoTunerApp::deviation_color(3.0), Color32::YELLOW);
        assert_eq!(PianoTunerApp::deviation_color(-30.0), Color32::RED);
    }
}
