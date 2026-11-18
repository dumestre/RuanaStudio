//! Estado e lógica de interação da tempo track: hover, arrasto/edição de pontos
//! de tempo (BPM), adição via duplo clique e remoção via Delete/ferramenta Erase.

use std::time::Instant;

use super::data::{TempoPoint, TempoTrackMetrics};

/// Distância (px) para o cursor "agarrar" um ponto de tempo (raio circular).
const TEMPO_POINT_GRAB_PX: f32 = 8.0;

/// Janela (ms) para um segundo clique contar como duplo clique.
const TEMPO_DOUBLE_MS: u128 = 350;

/// Estado de interação da tempo track (hover + arrasto + scrub).
#[derive(Clone, Debug)]
pub struct TempoTrackInteraction {
    pub hover_idx: Option<usize>,
    pub drag_idx: Option<usize>,
    pub drag_orig_beat: f32,
    pub drag_orig_bpm: f32,
    pub drag_start_cursor: (f32, f32),
    pub drag_live_beat: f32,
    pub drag_live_bpm: f32,
    pub drag_moved: bool,
    /// True enquanto o usuário arrasta o cursor na régua da tempo track.
    pub scrubbing: bool,
    /// Último clique no corpo da track: (instante, x) — duplo clique para adicionar ponto.
    pub last_press: Option<(Instant, f32)>,
}

impl Default for TempoTrackInteraction {
    fn default() -> Self {
        Self::new()
    }
}

impl TempoTrackInteraction {
    pub fn new() -> Self {
        Self {
            hover_idx: None,
            drag_idx: None,
            drag_orig_beat: 0.0,
            drag_orig_bpm: 120.0,
            drag_start_cursor: (0.0, 0.0),
            drag_live_beat: 0.0,
            drag_live_bpm: 120.0,
            drag_moved: false,
            scrubbing: false,
            last_press: None,
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.drag_idx.is_some()
    }

    /// Hit-test: qual ponto de tempo está sob o cursor (raio de 8px).
    pub fn hit_test(
        &self,
        points: &[TempoPoint],
        cursor: (f32, f32),
        m: &TempoTrackMetrics,
    ) -> Option<usize> {
        const RADIUS: f32 = TEMPO_POINT_GRAB_PX;
        for (i, p) in points.iter().enumerate() {
            let px = m.beat_to_x(p.beat);
            let py = m.bpm_to_y(p.bpm);
            let dx = cursor.0 - px;
            let dy = cursor.1 - py;
            if dx * dx + dy * dy <= RADIUS * RADIUS {
                return Some(i);
            }
        }
        None
    }

    /// Atualiza o hover conforme o cursor.
    pub fn update_hover(
        &mut self,
        points: &[TempoPoint],
        cursor: (f32, f32),
        m: &TempoTrackMetrics,
    ) {
        if self.is_dragging() {
            return;
        }
        self.hover_idx = self.hit_test(points, cursor, m);
    }

    /// Inicia o arrasto de um ponto de tempo.
    pub fn begin_drag(
        &mut self,
        points: &[TempoPoint],
        cursor: (f32, f32),
        m: &TempoTrackMetrics,
    ) -> bool {
        if let Some(idx) = self.hit_test(points, cursor, m) {
            let p = &points[idx];
            self.drag_idx = Some(idx);
            self.drag_orig_beat = p.beat;
            self.drag_orig_bpm = p.bpm;
            self.drag_start_cursor = cursor;
            self.drag_live_beat = p.beat;
            self.drag_live_bpm = p.bpm;
            self.drag_moved = false;
            true
        } else {
            false
        }
    }

    /// Move um ponto em arrasto. `snap` liga o grid musical (1/16) e BPM
    /// inteiro; sem snap, valor livre.
    pub fn drag_move(&mut self, cursor: (f32, f32), m: &TempoTrackMetrics, snap: bool) {
        if self.drag_idx.is_none() {
            return;
        }
        let dx = cursor.0 - self.drag_start_cursor.0;
        let dy = cursor.1 - self.drag_start_cursor.1;

        if !self.drag_moved && dx.abs() <= 3.0 && dy.abs() <= 3.0 {
            return;
        }
        self.drag_moved = true;

        let mut beat = self.drag_orig_beat + dx / m.px_per_beat.max(0.001);
        if snap {
            beat = (beat * 4.0).round() / 4.0; // snap to 1/4 (beat)
        }
        let mut bpm = self.y_to_bpm_from(self.drag_orig_bpm, dy, m);
        if snap {
            bpm = bpm.round();
        }
        self.drag_live_beat = beat.max(0.0);
        self.drag_live_bpm = bpm.clamp(super::data::BPM_MIN, super::data::BPM_MAX);
    }

    /// BPM alvo a partir de um deslocamento vertical `dy` em px.
    fn y_to_bpm_from(&self, orig_bpm: f32, dy: f32, m: &TempoTrackMetrics) -> f32 {
        let per_px = (super::data::BPM_MAX - super::data::BPM_MIN) / m.track_h.max(1.0);
        (orig_bpm - dy * per_px).clamp(super::data::BPM_MIN, super::data::BPM_MAX)
    }

    /// Finaliza o arrasto e aplica a nova posição.
    pub fn end_drag(&mut self, points: &mut [TempoPoint]) {
        if let Some(idx) = self.drag_idx {
            if self.drag_moved {
                if let Some(p) = points.get_mut(idx) {
                    p.beat = self.drag_live_beat.max(0.0);
                    p.bpm = self.drag_live_bpm.clamp(super::data::BPM_MIN, super::data::BPM_MAX);
                }
            }
        }
        self.drag_idx = None;
        self.drag_moved = false;
    }

    /// Registra um clique no corpo da track e devolve `true` quando é um
    /// DUPLO clique na mesma região (para criar um ponto de tempo).
    pub fn register_press(&mut self, x: f32) -> bool {
        let now = Instant::now();
        let is_double = match self.last_press {
            Some((t, last_x)) => {
                now.duration_since(t).as_millis() <= TEMPO_DOUBLE_MS
                    && (x - last_x).abs() <= 8.0
            }
            None => false,
        };
        self.last_press = if is_double { None } else { Some((now, x)) };
        is_double
    }

    /// Inicia scrub na régua da tempo track.
    pub fn begin_scrub(&mut self, cursor: (f32, f32)) {
        self.scrubbing = true;
        self.drag_start_cursor = cursor;
    }

    /// Move scrub - atualiza playhead position.
    pub fn scrub_move(&mut self, cursor: (f32, f32), m: &TempoTrackMetrics) -> f32 {
        let dx = cursor.0 - self.drag_start_cursor.0;
        let beat = self.drag_live_beat + dx / m.px_per_beat.max(0.001);
        self.drag_live_beat = beat.max(0.0);
        self.drag_live_beat
    }

    /// Finaliza scrub.
    pub fn end_scrub(&mut self) {
        self.scrubbing = false;
    }
}