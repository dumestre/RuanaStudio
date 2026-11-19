//! Estado e lógica de interação das lanes de automação: hover, arrasto/edição de pontos,
//! adição via duplo clique, remoção via Delete/ferramenta Erase, mudança de interpolação.

use std::time::Instant;

use daw_core::automation::{AutomationCurve, InterpolationType};

use super::{AutomationPointView, AutomationLaneMetrics};

/// Distância (px) para o cursor "agarrar" um ponto de automação (raio circular).
const AUTOMATION_POINT_GRAB_PX: f32 = 8.0;

/// Janela (ms) para um segundo clique contar como duplo clique.
const AUTOMATION_DOUBLE_MS: u128 = 350;

/// Estado de interação de uma lane de automação (hover + arrasto + scrub).
#[derive(Clone, Debug)]
pub struct AutomationInteraction {
    pub hover_idx: Option<usize>,
    pub drag_idx: Option<usize>,
    pub drag_orig_beat: f32,
    pub drag_orig_value: f32,
    pub drag_start_cursor: (f32, f32),
    pub drag_live_beat: f32,
    pub drag_live_value: f32,
    pub drag_moved: bool,
    /// True enquanto o usuário arrasta o cursor na régua da lane.
    pub scrubbing: bool,
    /// Último clique no corpo da lane: (instante, x) — duplo clique para adicionar ponto.
    pub last_press: Option<(Instant, f32)>,
    /// Hints de interpolação para renderização (índice -> tipo).
    pub interpolation_hints: Vec<InterpolationType>,
    /// Número de pontos (para renderização).
    pub points_len_hint: usize,
}

impl Default for AutomationInteraction {
    fn default() -> Self {
        Self::new()
    }
}

impl AutomationInteraction {
    pub fn new() -> Self {
        Self {
            hover_idx: None,
            drag_idx: None,
            drag_orig_beat: 0.0,
            drag_orig_value: 0.5,
            drag_start_cursor: (0.0, 0.0),
            drag_live_beat: 0.0,
            drag_live_value: 0.5,
            drag_moved: false,
            scrubbing: false,
            last_press: None,
            interpolation_hints: Vec::new(),
            points_len_hint: 0,
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.drag_idx.is_some()
    }

    /// Hit-test: qual ponto de automação está sob o cursor (raio de 8px).
    pub fn hit_test(
        &self,
        points: &[AutomationPointView],
        cursor: (f32, f32),
        m: &AutomationLaneMetrics,
    ) -> Option<usize> {
        const RADIUS: f32 = AUTOMATION_POINT_GRAB_PX;
        for (i, p) in points.iter().enumerate() {
            let px = m.beat_to_x(p.beat);
            let py = m.value_to_y(p.value);
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
        points: &[AutomationPointView],
        cursor: (f32, f32),
        m: &AutomationLaneMetrics,
    ) {
        if self.is_dragging() {
            return;
        }
        self.hover_idx = self.hit_test(points, cursor, m);
    }

    /// Inicia o arrasto de um ponto de automação.
    pub fn begin_drag(
        &mut self,
        points: &[AutomationPointView],
        cursor: (f32, f32),
        m: &AutomationLaneMetrics,
    ) -> bool {
        if let Some(idx) = self.hit_test(points, cursor, m) {
            let p = &points[idx];
            self.drag_idx = Some(idx);
            self.drag_orig_beat = p.beat;
            self.drag_orig_value = p.value;
            self.drag_start_cursor = cursor;
            self.drag_live_beat = p.beat;
            self.drag_live_value = p.value;
            self.drag_moved = false;
            true
        } else {
            false
        }
    }

    /// Move um ponto em arrasto. `snap` liga o grid musical (1/16) e valor quantizado.
    pub fn drag_move(&mut self, cursor: (f32, f32), m: &AutomationLaneMetrics, snap: bool) {
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

        // Valor: dy negativo = valor aumenta (y sobe = valor sobe)
        let per_px = 1.0 / m.lane_h.max(1.0);
        let mut value = self.drag_orig_value - dy * per_px;
        if snap {
            value = (value * 128.0).round() / 128.0; // snap to 1/128
        }
        self.drag_live_beat = beat.max(0.0);
        self.drag_live_value = value.clamp(0.0, 1.0);
    }

    /// Finaliza o arrasto e retorna (idx, new_beat, new_value) se mudou.
    pub fn end_drag(&mut self) -> Option<(usize, f32, f32)> {
        if let Some(idx) = self.drag_idx {
            if self.drag_moved {
                let beat = self.drag_live_beat.max(0.0);
                let value = self.drag_live_value.clamp(0.0, 1.0);
                self.drag_idx = None;
                self.drag_moved = false;
                return Some((idx, beat, value));
            }
        }
        self.drag_idx = None;
        self.drag_moved = false;
        None
    }

    /// Registra um clique no corpo da lane e devolve `true` quando é um
    /// DUPLO clique na mesma região (para criar um ponto de automação).
    pub fn register_press(&mut self, x: f32) -> bool {
        let now = Instant::now();
        let is_double = match self.last_press {
            Some((t, last_x)) => {
                now.duration_since(t).as_millis() <= AUTOMATION_DOUBLE_MS
                    && (x - last_x).abs() <= 8.0
            }
            None => false,
        };
        self.last_press = if is_double { None } else { Some((now, x)) };
        is_double
    }

    /// Inicia scrub na régua da lane.
    pub fn begin_scrub(&mut self, cursor: (f32, f32)) {
        self.scrubbing = true;
        self.drag_start_cursor = cursor;
    }

    /// Move scrub - atualiza playhead position.
    pub fn scrub_move(&mut self, cursor: (f32, f32), m: &AutomationLaneMetrics) -> f32 {
        let dx = cursor.0 - self.drag_start_cursor.0;
        let beat = self.drag_live_beat + dx / m.px_per_beat.max(0.001);
        self.drag_live_beat = beat.max(0.0);
        self.drag_live_beat
    }

    /// Finaliza scrub.
    pub fn end_scrub(&mut self) {
        self.scrubbing = false;
    }

    /// Atualiza hints de interpolação a partir da curva.
    pub fn update_interpolation_hints(&mut self, curve: &AutomationCurve) {
        self.interpolation_hints = curve.points.iter().map(|p| p.interpolation).collect();
        self.points_len_hint = curve.points.len();
    }

    /// Alterna a interpolação do ponto em idx.
    pub fn toggle_interpolation(&mut self, idx: usize) -> Option<InterpolationType> {
        if idx < self.interpolation_hints.len() {
            let new = match self.interpolation_hints[idx] {
                InterpolationType::Linear => InterpolationType::Step,
                InterpolationType::Step => InterpolationType::Bezier,
                InterpolationType::Bezier => InterpolationType::Linear,
            };
            self.interpolation_hints[idx] = new;
            Some(new)
        } else {
            None
        }
    }
}