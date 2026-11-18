//! Geração de instâncias da tempo track: fundo, grid, gutter, curva de tempo
//! (polilinhas lineares entre eventos) e marcadores de ponto de tempo.

use daw_core::timeline::TempoMap;

use crate::{palette, RectInstance};
use super::{TempoTrackInteraction, TempoTrackMetrics, TempoPoint};

/// Gera as instâncias completas da tempo track (fundo + grid + gutter + curva + pontos).
pub fn build_tempo_track_instances(
    tempo_map: &TempoMap,
    inter: &TempoTrackInteraction,
    m: &TempoTrackMetrics,
    width: f32,
) -> Vec<RectInstance> {
    let mut instances = Vec::new();

    // 1. Fundo + grid vertical
    instances.extend(m.build_background_instances(width));

    // 2. Gutter esquerdo (label "TEMPO")
    instances.extend(m.build_gutter_instances());

    // 3. Converte o mapa de tempo em pontos ordenados.
    let mut points: Vec<TempoPoint> = tempo_map
        .changes
        .iter()
        .map(|c| TempoPoint { beat: c.beat, bpm: c.bpm })
        .collect();
    points.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap());

    // 4. Curva: segmentos lineares (rampas) entre eventos consecutivos.
    push_curve(&mut instances, &points, m, width);

    // 5. Linhas verticais finas de cada evento até a base da track.
    for p in &points {
        let x = m.beat_to_x(p.beat);
        if x < 0.0 || x > width {
            continue;
        }
        let y = m.bpm_to_y(p.bpm);
        instances.push(RectInstance {
            x, y,
            w: 1.0, h: (m.track_top + m.track_h - y).max(0.0),
            color: [palette::BASE[m.theme.clamp(0, 6)].0, palette::BASE[m.theme.clamp(0, 6)].1, palette::BASE[m.theme.clamp(0, 6)].2, 90],
            wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
        });
    }

    // 6. Marcadores (pontos de tempo).
    for (i, p) in points.iter().enumerate() {
        push_point_marker(&mut instances, p, i, inter, m);
    }

    instances
}

/// Desenha a polilinha da curva de tempo com tiles de ~3px.
fn push_curve(
    instances: &mut Vec<RectInstance>,
    points: &[TempoPoint],
    m: &TempoTrackMetrics,
    width: f32,
) {
    if points.is_empty() {
        return;
    }
    let col = palette::ACCENT[m.theme.clamp(0, 6)];
    let track_top = m.track_top;
    let track_bottom = m.track_top + m.track_h;

    for w in points.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let (x0, y0) = (m.beat_to_x(a.beat), m.bpm_to_y(a.bpm));
        let (x1, y1) = (m.beat_to_x(b.beat), m.bpm_to_y(b.bpm));

        // Culling do segmento inteiro (fora do viewport).
        if (x0 < 0.0 && x1 < 0.0) || (x0 > width && x1 > width) {
            continue;
        }

        let span = (x1 - x0).abs().max(1.0);
        let steps = (span / 3.0).ceil() as usize;
        for s in 0..=steps {
            let f = s as f32 / steps as f32;
            let x = x0 + (x1 - x0) * f;
            if x < 0.0 || x > width {
                continue;
            }
            let y = y0 + (y1 - y0) * f;
            if y < track_top || y > track_bottom {
                continue;
            }
            instances.push(RectInstance {
                x, y: y - 1.0,
                w: 3.0, h: 2.0,
                color: [col.0, col.1, col.2, 255],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });
        }
    }
}

/// Desenha um marcador de ponto de tempo (quadrado com radius, como as notas
/// do piano roll) com estado de hover/seleção.
fn push_point_marker(
    instances: &mut Vec<RectInstance>,
    p: &TempoPoint,
    idx: usize,
    inter: &TempoTrackInteraction,
    m: &TempoTrackMetrics,
) {
    let t = m.theme.clamp(0, 6);
    let x = m.beat_to_x(p.beat) - 5.0;
    let y = m.bpm_to_y(p.bpm) - 5.0;
    if y + 10.0 < m.track_top || y > m.track_top + m.track_h {
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
}