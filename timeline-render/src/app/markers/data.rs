//! Marcadores na régua da timeline: posição, regiões, loops, CD tracks.

use crate::{palette, rgba, RectInstance};
use daw_core::timeline::{Marker, MarkerType};

/// Métricas para renderização de marcadores na régua.
#[derive(Clone, Copy, Debug)]
pub struct MarkerMetrics {
    pub px_per_beat: f32,
    pub scroll_x: f32,
    pub ruler_top: f32,
    pub ruler_height: f32,
    pub theme: usize,
}

impl MarkerMetrics {
    /// Converte beat para coordenada X na tela.
    pub fn beat_to_x(&self, beat: f32) -> f32 {
        beat * self.px_per_beat - self.scroll_x
    }

    /// Verifica se o marcador está visível no viewport.
    pub fn is_visible(&self, x: f32, width: f32) -> bool {
        x + 8.0 >= 0.0 && x <= width
    }
}

/// Gera instâncias para um marcador individual.
pub fn build_marker_instances(
    marker: &Marker,
    metrics: &MarkerMetrics,
    width: f32,
    hovered: bool,
    selected: bool,
) -> Vec<RectInstance> {
    let t = metrics.theme.clamp(0, 6) as usize;
    let mut instances = Vec::new();
    let x = metrics.beat_to_x(marker.beat);

    if !metrics.is_visible(x, width) {
        return instances;
    }

    let ruler_bottom = metrics.ruler_top + metrics.ruler_height;
    let marker_color = palette::marker_color(marker.color, t);

    match marker.marker_type {
        MarkerType::Position => {
            // Triângulo apontando para baixo na régua
            let tri_size = if hovered || selected { 12.0 } else { 10.0 };
            let tri_y = ruler_bottom - 2.0;

            // Desenha triângulo como retângulo rotacionado (simplificado: losango)
            instances.push(RectInstance {
                x: x - tri_size / 2.0,
                y: tri_y - tri_size,
                w: tri_size,
                h: tri_size,
                color: [marker_color.0, marker_color.1, marker_color.2, if selected { 255 } else { 200 }],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });

            // Linha vertical descendo da régua até o meio da timeline
            instances.push(RectInstance {
                x: x - 0.5,
                y: ruler_bottom,
                w: 1.0,
                h: 50.0,
                color: [marker_color.0, marker_color.1, marker_color.2, 100],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });
        }

        MarkerType::RegionStart | MarkerType::LoopStart => {
            // Marcador de início: triângulo + linha vertical + label
            let tri_size = if hovered || selected { 14.0 } else { 12.0 };
            let tri_y = ruler_bottom - 2.0;

            instances.push(RectInstance {
                x: x - tri_size / 2.0,
                y: tri_y - tri_size,
                w: tri_size,
                h: tri_size,
                color: [marker_color.0, marker_color.1, marker_color.2, 255],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });

            // Linha vertical
            instances.push(RectInstance {
                x: x - 1.0,
                y: ruler_bottom,
                w: 2.0,
                h: 80.0,
                color: [marker_color.0, marker_color.1, marker_color.2, 180],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });
        }

        MarkerType::RegionEnd | MarkerType::LoopEnd => {
            // Marcador de fim: triângulo invertido
            let tri_size = if hovered || selected { 14.0 } else { 12.0 };
            let tri_y = ruler_bottom - 2.0;

            instances.push(RectInstance {
                x: x - tri_size / 2.0,
                y: tri_y - tri_size,
                w: tri_size,
                h: tri_size,
                color: [marker_color.0, marker_color.1, marker_color.2, 255],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });

            // Linha vertical
            instances.push(RectInstance {
                x: x - 1.0,
                y: ruler_bottom,
                w: 2.0,
                h: 80.0,
                color: [marker_color.0, marker_color.1, marker_color.2, 180],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });
        }

        MarkerType::CDTrack => {
            // Marcador CD: diamante com número da faixa
            let dia_size = if hovered || selected { 16.0 } else { 14.0 };
            let dia_y = ruler_bottom - 2.0;

            instances.push(RectInstance {
                x: x - dia_size / 2.0,
                y: dia_y - dia_size,
                w: dia_size,
                h: dia_size,
                color: [marker_color.0, marker_color.1, marker_color.2, 255],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });

            // Linha vertical mais curta
            instances.push(RectInstance {
                x: x - 1.0,
                y: ruler_bottom,
                w: 2.0,
                h: 40.0,
                color: [marker_color.0, marker_color.1, marker_color.2, 150],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });
        }
    }

    instances
}

/// Gera instâncias para a região/loop entre dois marcadores.
pub fn build_region_instances(
    start_marker: &Marker,
    end_marker: &Marker,
    metrics: &MarkerMetrics,
    width: f32,
    timeline_bottom: f32,
    selected: bool,
) -> Vec<RectInstance> {
    let t = metrics.theme.clamp(0, 6) as usize;
    let mut instances = Vec::new();

    let start_x = metrics.beat_to_x(start_marker.beat);
    let end_x = metrics.beat_to_x(end_marker.end_beat_or_self());
    let ruler_bottom = metrics.ruler_top + metrics.ruler_height;

    let region_left = start_x.min(end_x);
    let region_right = start_x.max(end_x);
    let region_width = region_right - region_left;

    if region_width <= 0.0 || region_left > width || region_right < 0.0 {
        return instances;
    }

    let region_color = palette::marker_color(start_marker.color, t);
    let alpha = if selected { 60 } else { 30 };

    // Preenchimento da região (da régua até o fim da timeline visível)
    instances.push(RectInstance {
        x: region_left.max(0.0),
        y: ruler_bottom,
        w: (region_right - region_left).min(width - region_left.max(0.0)).max(0.0),
        h: (timeline_bottom - ruler_bottom).max(0.0),
        color: [region_color.0, region_color.1, region_color.2, alpha],
        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
    });

    // Bordas da região
    let border_alpha = if selected { 180 } else { 120 };
    instances.push(RectInstance {
        x: region_left,
        y: ruler_bottom,
        w: 2.0,
        h: (timeline_bottom - ruler_bottom).max(0.0),
        color: [region_color.0, region_color.1, region_color.2, border_alpha],
        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
    });
    instances.push(RectInstance {
        x: region_right - 2.0,
        y: ruler_bottom,
        w: 2.0,
        h: (timeline_bottom - ruler_bottom).max(0.0),
        color: [region_color.0, region_color.1, region_color.2, border_alpha],
        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
    });

    instances
}

/// Constrói todas as instâncias de marcadores para a régua.
pub fn build_all_marker_instances(
    markers: &[Marker],
    metrics: &MarkerMetrics,
    width: f32,
    timeline_bottom: f32,
    hovered_id: Option<u64>,
    selected_id: Option<u64>,
) -> Vec<RectInstance> {
    let mut instances = Vec::new();

    // Primeiro: regiões/loops (por baixo dos marcadores)
    let mut processed_regions = std::collections::HashSet::new();
    for marker in markers {
        if marker.is_region_start() {
            if let Some(end_marker) = markers.iter().find(|m| m.id == marker.id && m.is_region_end()) {
                if processed_regions.insert(marker.id) {
                    instances.extend(build_region_instances(
                        marker,
                        end_marker,
                        metrics,
                        width,
                        timeline_bottom,
                        selected_id == Some(marker.id),
                    ));
                }
            }
        }
    }

    // Segundo: marcadores individuais
    for marker in markers {
        let hovered = hovered_id == Some(marker.id);
        let selected = selected_id == Some(marker.id);
        instances.extend(build_marker_instances(marker, metrics, width, hovered, selected));
    }

    instances
}