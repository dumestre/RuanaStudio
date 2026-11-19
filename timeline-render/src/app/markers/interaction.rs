//! Interação com marcadores na régua: hover, drag, criação.

use crate::app::markers::data::MarkerMetrics;
use daw_core::timeline::{Marker, MarkerType};

/// Estado de interação com marcadores.
#[derive(Clone, Debug, Default)]
pub struct MarkerInteraction {
    /// ID do marcador sob o mouse (hover)
    pub hovered_id: Option<u64>,
    /// ID do marcador sendo arrastado
    pub dragging_id: Option<u64>,
    /// Offset do cursor em relação ao marcador ao iniciar drag
    pub drag_offset_x: f32,
    /// Tipo de drag: movendo marcador, redimensionando região, etc.
    pub drag_mode: MarkerDragMode,
    /// Se está criando uma nova região (primeiro clique = início)
    pub creating_region: bool,
    /// Beat inicial da região sendo criada
    pub region_start_beat: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum MarkerDragMode {
    #[default]
    None,
    /// Movendo um marcador de posição
    MovePosition,
    /// Movendo início de região/loop
    MoveRegionStart,
    /// Movendo fim de região/loop
    MoveRegionEnd,
    /// Movendo loop inteiro (início + fim)
    MoveLoop,
    /// Redimensionando região (arrastando borda)
    ResizeRegion,
}

impl MarkerInteraction {
    pub fn new() -> Self {
        Self::default()
    }

    /// Atualiza hover baseado na posição do mouse.
    pub fn update_hover(&mut self, markers: &[Marker], pos: (f32, f32), metrics: &MarkerMetrics, width: f32) {
        if self.dragging_id.is_some() {
            return; // Não atualiza hover durante drag
        }

        let x = pos.0;
        let ruler_bottom = metrics.ruler_top + metrics.ruler_height;

        // Verifica se está na área da régua
        if pos.1 < metrics.ruler_top || pos.1 > ruler_bottom + 20.0 {
            self.hovered_id = None;
            return;
        }

        // Procura marcador mais próximo do cursor (hit test)
        let mut best_id = None;
        let mut best_dist = f32::INFINITY;

        for marker in markers {
            let marker_x = metrics.beat_to_x(marker.beat);
            let dist = (marker_x - x).abs();

            // Hit area maior para marcadores de região/loop
            let hit_radius = match marker.marker_type {
                MarkerType::Position => 10.0,
                MarkerType::RegionStart | MarkerType::RegionEnd |
                MarkerType::LoopStart | MarkerType::LoopEnd => 14.0,
                MarkerType::CDTrack => 16.0,
            };

            if dist < hit_radius && dist < best_dist {
                best_dist = dist;
                best_id = Some(marker.id);
            }
        }

        self.hovered_id = best_id;
    }

    /// Inicia drag de um marcador.
    pub fn begin_drag(&mut self, markers: &[Marker], pos: (f32, f32), metrics: &MarkerMetrics) -> bool {
        if let Some(id) = self.hovered_id {
            if let Some(marker) = markers.iter().find(|m| m.id == id) {
                self.dragging_id = Some(id);
                self.drag_offset_x = pos.0 - metrics.beat_to_x(marker.beat);

                self.drag_mode = match marker.marker_type {
                    MarkerType::Position => MarkerDragMode::MovePosition,
                    MarkerType::RegionStart => MarkerDragMode::MoveRegionStart,
                    MarkerType::RegionEnd => MarkerDragMode::MoveRegionEnd,
                    MarkerType::LoopStart => MarkerDragMode::MoveLoop,
                    MarkerType::LoopEnd => MarkerDragMode::MoveLoop,
                    MarkerType::CDTrack => MarkerDragMode::MovePosition,
                };
                return true;
            }
        }
        false
    }

    /// Move o marcador durante o drag.
    pub fn drag_move(&mut self, pos: (f32, f32), metrics: &MarkerMetrics, snap_enabled: bool, markers: &mut [Marker]) {
        if let Some(id) = self.dragging_id {
            let new_x = pos.0 - self.drag_offset_x;
            let mut new_beat = new_x / metrics.px_per_beat + metrics.scroll_x / metrics.px_per_beat;
            new_beat = new_beat.max(0.0);

            if snap_enabled {
                new_beat = (new_beat * 4.0).round() / 4.0; // Snap para 1/4 nota
            }

            // Aplica movimento baseado no modo
            match self.drag_mode {
                MarkerDragMode::MovePosition => {
                    if let Some(m) = markers.iter_mut().find(|m| m.id == id) {
                        m.beat = new_beat;
                    }
                }
                MarkerDragMode::MoveRegionStart => {
                    if let Some(m) = markers.iter_mut().find(|m| m.id == id) {
                        let old_beat = m.beat;
                        m.beat = new_beat;
                        // Move o fim da região junto se necessário (mantém duração)
                        if let Some(end_m) = markers.iter_mut().find(|m| m.id == id && m.is_region_end()) {
                            let duration = end_m.beat - old_beat;
                            end_m.beat = new_beat + duration;
                        }
                    }
                }
                MarkerDragMode::MoveRegionEnd => {
                    // Encontra o beat de início antes do mutable borrow
                    let start_beat = markers.iter()
                        .find(|m| m.id == id && m.is_region_start())
                        .map(|s| s.beat + 0.25)
                        .unwrap_or(0.25);
                    
                    if let Some(m) = markers.iter_mut().find(|m| m.id == id && m.is_region_end()) {
                        m.beat = new_beat.max(start_beat);
                    }
                }
                MarkerDragMode::MoveLoop => {
                    // Move loop inteiro
                    let start_marker = markers.iter().find(|m| m.id == id && m.marker_type == MarkerType::LoopStart).cloned();
                    let end_marker = markers.iter().find(|m| m.id == id && m.marker_type == MarkerType::LoopEnd).cloned();
                    
                    if let (Some(start), Some(end)) = (start_marker, end_marker) {
                        let duration = end.beat - start.beat;
                        let new_start = new_beat;
                        let new_end = new_start + duration;
                        
                        for m in markers.iter_mut() {
                            if m.id == id {
                                if m.marker_type == MarkerType::LoopStart {
                                    m.beat = new_start;
                                } else if m.marker_type == MarkerType::LoopEnd {
                                    m.beat = new_end;
                                }
                            }
                        }
                    }
                }
                MarkerDragMode::ResizeRegion => {
                    // Encontra o beat de início antes do mutable borrow
                    let start_beat = markers.iter()
                        .find(|m| m.id == id && m.is_region_start())
                        .map(|s| s.beat)
                        .unwrap_or(0.0);
                    
                    if let Some(m) = markers.iter_mut().find(|m| m.id == id && m.is_region_end()) {
                        m.beat = new_beat.max(start_beat + 0.25);
                    }
                }
                MarkerDragMode::None => {}
            }

            // Reordena marcadores por beat
            markers.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        }
    }

    /// Finaliza drag.
    pub fn end_drag(&mut self) {
        self.dragging_id = None;
        self.drag_mode = MarkerDragMode::None;
        self.drag_offset_x = 0.0;
    }

    /// Inicia criação de região (primeiro clique define início).
    pub fn begin_region_create(&mut self, beat: f32) {
        self.creating_region = true;
        self.region_start_beat = Some(beat);
    }

    /// Finaliza criação de região (segundo clique define fim).
    pub fn end_region_create(&mut self, beat: f32) -> Option<(f32, f32)> {
        if self.creating_region {
            if let Some(start) = self.region_start_beat {
                let start_beat = start.min(beat);
                let end_beat = start.max(beat);
                if end_beat - start_beat > 0.25 {
                    self.creating_region = false;
                    self.region_start_beat = None;
                    return Some((start_beat, end_beat));
                }
            }
            self.creating_region = false;
            self.region_start_beat = None;
        }
        None
    }

    /// Verifica se está arrastando.
    pub fn is_dragging(&self) -> bool {
        self.dragging_id.is_some()
    }

    /// Verifica se está criando região.
    pub fn is_creating_region(&self) -> bool {
        self.creating_region
    }
}

/// Hit test para encontrar marcador sob o cursor.
pub fn hit_test_marker(markers: &[Marker], pos: (f32, f32), metrics: &MarkerMetrics) -> Option<u64> {
    let x = pos.0;
    let ruler_bottom = metrics.ruler_top + metrics.ruler_height;

    if pos.1 < metrics.ruler_top || pos.1 > ruler_bottom + 20.0 {
        return None;
    }

    for marker in markers {
        let marker_x = metrics.beat_to_x(marker.beat);
        let dist = (marker_x - x).abs();

        let hit_radius = match marker.marker_type {
            MarkerType::Position => 10.0,
            MarkerType::RegionStart | MarkerType::RegionEnd |
            MarkerType::LoopStart | MarkerType::LoopEnd => 14.0,
            MarkerType::CDTrack => 16.0,
        };

        if dist < hit_radius {
            return Some(marker.id);
        }
    }
    None
}