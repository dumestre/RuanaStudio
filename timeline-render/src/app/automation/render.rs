//! Geração de instâncias das lanes de automação: fundo, grid, gutter, curva
//! (polilinhas com interpolação linear/step/bezier) e marcadores de ponto.

use daw_core::automation::{AutomationCurve, InterpolationType};

use crate::{palette, RectInstance};
use super::{AutomationInteraction, AutomationLaneMetrics, AutomationPointView};

/// Gera as instâncias completas de uma lane de automação (fundo + grid + gutter + curva + pontos).
pub fn build_automation_lane_instances(
    curve: &AutomationCurve,
    inter: &AutomationInteraction,
    m: &AutomationLaneMetrics,
    width: f32,
) -> Vec<RectInstance> {
    let mut instances = Vec::new();

    // 1. Fundo + grid vertical
    instances.extend(m.build_background_instances(width));

    // 2. Gutter esquerdo (label do parâmetro)
    instances.extend(m.build_gutter_instances());

    // 3. Converte curva em pontos de view.
    let points = curve_to_points(curve);

    // 4. Curva: segmentos entre pontos consecutivos.
    push_curve(&mut instances, &points, &curve.param.display_name(), inter, m, width);

    // 5. Marcadores (pontos de automação).
    for (i, p) in points.iter().enumerate() {
        push_point_marker(&mut instances, p, i, inter, m);
    }

    instances
}

/// Converte AutomationCurve do core para pontos de view (versão standalone).
fn curve_to_points(curve: &AutomationCurve) -> Vec<AutomationPointView> {
    curve.points.iter().map(|p| AutomationPointView {
        beat: p.beat,
        value: p.value,
        interpolation: p.interpolation,
    }).collect()
}

/// Desenha a polilinha da curva de automação com tiles de ~3px, respeitando interpolação.
fn push_curve(
    instances: &mut Vec<RectInstance>,
    points: &[AutomationPointView],
    _param_name: &str,
    inter: &AutomationInteraction,
    m: &AutomationLaneMetrics,
    width: f32,
) {
    if points.is_empty() {
        return;
    }
    let t = m.theme.clamp(0, 6);
    let col = palette::ACCENT[t];
    let lane_top = m.lane_top;
    let lane_bottom = m.lane_top + m.lane_h;

    for w in points.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let (x0, y0) = (m.beat_to_x(a.beat), m.value_to_y(a.value));
        let (x1, y1) = (m.beat_to_x(b.beat), m.value_to_y(b.value));

        // Culling do segmento inteiro (fora do viewport).
        if (x0 < 0.0 && x1 < 0.0) || (x0 > width && x1 > width) {
            continue;
        }

        let span = (x1 - x0).abs().max(1.0);
        let steps = (span / 3.0).ceil() as usize;

        for s in 0..=steps {
            let f = s as f32 / steps as f32;

            let (x, y) = match a.interpolation {
                InterpolationType::Linear => {
                    (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f)
                }
                InterpolationType::Step => {
                    // Step: mantém valor de 'a' até o próximo ponto
                    if f < 1.0 {
                        (x0 + (x1 - x0) * f, y0)
                    } else {
                        (x1, y1)
                    }
                }
                InterpolationType::Bezier => {
                    // Simplified: linear for now
                    (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f)
                }
            };

            if x < 0.0 || x > width {
                continue;
            }
            if y < lane_top || y > lane_bottom {
                continue;
            }

            // Highlight se ponto está sendo arrastado
            let is_dragged = inter.drag_idx == Some(points.iter().position(|p| p.beat == a.beat && p.value == a.value).unwrap_or(0));
            let alpha = if is_dragged { 255 } else { 200 };

            instances.push(RectInstance {
                x, y: y - 1.0,
                w: 3.0, h: 2.0,
                color: [col.0, col.1, col.2, alpha],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });
        }
    }
}

/// Desenha um marcador de ponto de automação (quadrado com radius) com estado de hover/seleção.
fn push_point_marker(
    instances: &mut Vec<RectInstance>,
    p: &AutomationPointView,
    idx: usize,
    inter: &AutomationInteraction,
    m: &AutomationLaneMetrics,
) {
    let t = m.theme.clamp(0, 6);
    let x = m.beat_to_x(p.beat) - 5.0;
    let y = m.value_to_y(p.value) - 5.0;
    if y + 10.0 < m.lane_top || y > m.lane_top + m.lane_h {
        return;
    }

    let is_dragging = inter.drag_idx == Some(idx);
    let is_hovered = inter.hover_idx == Some(idx);
    let selected = is_dragging || is_hovered;

    let base = if selected {
        palette::ACCENT[t]
    } else {
        palette::CLIP_MIDI_DEFAULT[t]
    };

    // Corpo do marcador
    instances.push(RectInstance {
        x, y, w: 10.0, h: 10.0,
        color: [base.0, base.1, base.2, 255],
        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 2.0,
    });

    // Borda
    let border_col = if selected { [0xFF, 0xFF, 0xFF, 255] } else { [0x1A, 0x1A, 0x1A, 255] };
    instances.push(RectInstance {
        x, y, w: 10.0, h: 1.0, color: border_col,
        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
    });
    instances.push(RectInstance {
        x, y: y + 9.0, w: 10.0, h: 1.0, color: border_col,
        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
    });
    instances.push(RectInstance {
        x, y, w: 1.0, h: 10.0, color: border_col,
        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
    });
    instances.push(RectInstance {
        x: x + 9.0, y, w: 1.0, h: 10.0, color: border_col,
        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
    });

    // Indicador de interpolação (pequeno ícone no canto)
    if idx < points_len_hint(inter) {
        let interp_col = if selected { [0xFF, 0xFF, 0x00, 255] } else { [0x88, 0x88, 0x88, 200] };
        let icon_x = x + 10.0;
        let icon_y = y - 2.0;
        match inter.interpolation_hint(idx) {
            Some(InterpolationType::Step) => {
                // Degrau
                instances.push(RectInstance {
                    x: icon_x, y: icon_y, w: 6.0, h: 1.0, color: interp_col,
                    wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
                });
                instances.push(RectInstance {
                    x: icon_x + 5.0, y: icon_y - 4.0, w: 1.0, h: 8.0, color: interp_col,
                    wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
                });
            }
            Some(InterpolationType::Bezier) => {
                // Curva ~
                instances.push(RectInstance {
                    x: icon_x, y: icon_y + 1.0, w: 6.0, h: 1.0, color: interp_col,
                    wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
                });
            }
            _ => {} // Linear: sem ícone
        }
    }
}

// Helper para evitar borrow issues
fn points_len_hint(inter: &AutomationInteraction) -> usize {
    inter.points_len_hint
}

// Helper trait para interpolation hint
trait InterpolationHint {
    fn interpolation_hint(&self, idx: usize) -> Option<InterpolationType>;
}

impl InterpolationHint for AutomationInteraction {
    fn interpolation_hint(&self, idx: usize) -> Option<InterpolationType> {
        self.interpolation_hints.get(idx).copied()
    }
}