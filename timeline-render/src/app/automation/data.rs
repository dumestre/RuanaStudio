//! Geometria das lanes de automação: faixa dedicada abaixo de cada track header.
//!
//! Cada track pode ter múltiplas lanes de automação (volume, pan, plugin params).
//! A lane mostra a curva de automação (pontos + interpolação) e permite editar.

use crate::{palette, rgba, RectInstance};
use daw_core::automation::{AutomationCurve, InterpolationType};

/// Altura fixa de uma lane de automação (px lógicos).
pub const AUTOMATION_LANE_HEIGHT: f32 = 40.0;

/// Espaçamento entre lanes de automação.
pub const AUTOMATION_LANE_GAP: f32 = 2.0;

/// Um ponto de automação para renderização (espelha AutomationPoint).
#[derive(Clone, Debug)]
pub struct AutomationPointView {
    pub beat: f32,
    pub value: f32, // 0.0..1.0 normalizado
    pub interpolation: InterpolationType,
}

/// Conversões cursor ↔ conteúdo da lane de automação.
#[derive(Clone)]
pub struct AutomationLaneMetrics {
    pub px_per_beat: f32,
    pub scroll_x: f32,
    /// Topo da lane (coordenada Y global da janela).
    pub lane_top: f32,
    /// Altura da lane.
    pub lane_h: f32,
    pub theme: usize,
    /// Parâmetro sendo automatizado (para label no gutter).
    pub param_name: String,
    /// Cor da lane (palette index).
    pub color_index: u8,
}

impl AutomationLaneMetrics {
    /// Converte beat → coordenada de janela.
    pub fn beat_to_x(&self, beat: f32) -> f32 {
        beat * self.px_per_beat - self.scroll_x
    }

    /// Converte x de janela → beat.
    pub fn x_to_beat(&self, x: f32) -> f32 {
        (x + self.scroll_x) / self.px_per_beat.max(0.001)
    }

    /// Converte valor normalizado 0..1 → coordenada de janela (valor alto no topo).
    pub fn value_to_y(&self, value: f32) -> f32 {
        let norm = (1.0 - value.clamp(0.0, 1.0));
        self.lane_top + norm * self.lane_h
    }

    /// Converte y de janela → valor normalizado 0..1.
    pub fn y_to_value(&self, y: f32) -> f32 {
        let rel = ((y - self.lane_top) / self.lane_h.max(1.0)).clamp(0.0, 1.0);
        1.0 - rel
    }

    /// Converte cursor em (beat, value).
    pub fn cursor_to_beat_value(&self, cursor: (f32, f32)) -> (f32, f32) {
        (self.x_to_beat(cursor.0).max(0.0), self.y_to_value(cursor.1))
    }

    /// Retorna as instâncias de fundo da lane (fundo + grid vertical).
    pub fn build_background_instances(&self, width: f32) -> Vec<RectInstance> {
        let t = self.theme.clamp(0, 6);
        let mut instances = Vec::new();

        let lane_top = self.lane_top;
        let lane_h = self.lane_h.max(0.0);
        let lane_bottom = lane_top + lane_h;

        // Fundo base da lane
        instances.push(RectInstance {
            x: 0.0,
            y: lane_top,
            w: width,
            h: lane_h,
            color: rgba(palette::TIMELINE_BG[t], 255),
            wave_offset: 0,
            wave_count: 0,
            clip_w: 0.0,
            clip_h: 0.0,
            radius: 0.0,
        });

        // Grid VERTICAL — subdivisões (mesmas regras de zoom da timeline).
        let px_per_bar = self.px_per_beat * 4.0;
        let total_bars = ((width + self.scroll_x) / px_per_bar).ceil() as usize + 1;
        for i in 0..total_bars {
            let bx = i as f32 * px_per_bar - self.scroll_x;
            if bx > width + px_per_bar || bx < -px_per_bar { continue; }

            // 1/64
            if px_per_bar >= 960.0 {
                let n = 64u32;
                let step = px_per_bar / n as f32;
                for m in 1..n {
                    let xs = bx + m as f32 * step;
                    if xs < 0.0 || xs > width { continue; }
                    instances.push(RectInstance {
                        x: xs, y: lane_top, w: 1.0, h: lane_h,
                        color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 70],
                        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
                    });
                }
            }
            // 1/32
            if px_per_bar >= 480.0 {
                let n = 32u32;
                let step = px_per_bar / n as f32;
                for m in 1..n {
                    let xs = bx + m as f32 * step;
                    if xs < 0.0 || xs > width { continue; }
                    instances.push(RectInstance {
                        x: xs, y: lane_top, w: 1.0, h: lane_h,
                        color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 90],
                        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
                    });
                }
            }
            // 1/16
            if px_per_bar >= 240.0 {
                let n = 16u32;
                let step = px_per_bar / n as f32;
                for m in 1..n {
                    let xs = bx + m as f32 * step;
                    if xs < 0.0 || xs > width { continue; }
                    instances.push(RectInstance {
                        x: xs, y: lane_top, w: 1.0, h: lane_h,
                        color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 110],
                        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
                    });
                }
            }
            // 1/8
            if px_per_bar >= 120.0 {
                let n = 8u32;
                let step = px_per_bar / n as f32;
                for m in 1..n {
                    let xs = bx + m as f32 * step;
                    if xs < 0.0 || xs > width { continue; }
                    instances.push(RectInstance {
                        x: xs, y: lane_top, w: 1.0, h: lane_h,
                        color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 130],
                        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
                    });
                }
            }
            // Batidas (1/4) — mais grossas, por cima das subdivisões
            if px_per_bar >= 40.0 {
                let n = 4u32;
                let step = px_per_bar / n as f32;
                for m in 1..n {
                    let xs = bx + m as f32 * step;
                    if xs < 0.0 || xs > width { continue; }
                    instances.push(RectInstance {
                        x: xs, y: lane_top, w: 1.0, h: lane_h,
                        color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 150],
                        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
                    });
                }
            }
        }

        // Linha central (valor 0.5)
        let center_y = self.value_to_y(0.5);
        if center_y >= lane_top && center_y <= lane_bottom {
            instances.push(RectInstance {
                x: 0.0, y: center_y - 0.5, w: width, h: 1.0,
                color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 100],
                wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
            });
        }

        // Linha separadora inferior da lane
        instances.push(RectInstance {
            x: 0.0, y: lane_bottom - 1.0, w: width, h: 1.0,
            color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 255],
            wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
        });

        instances
    }

    /// Instâncias do gutter esquerdo com o label do parâmetro.
    pub fn build_gutter_instances(&self) -> Vec<RectInstance> {
        use crate::build_text_instances;
        let t = self.theme.clamp(0, 6);
        let mut instances = Vec::new();
        let lane_top = self.lane_top;
        let lane_h = self.lane_h.max(0.0);

        // Fundo do gutter
        let gutter_w = 80.0;
        instances.push(RectInstance {
            x: 0.0,
            y: lane_top,
            w: gutter_w,
            h: lane_h,
            color: rgba(palette::BASE[t], 255),
            wave_offset: 0,
            wave_count: 0,
            clip_w: 0.0,
            clip_h: 0.0,
            radius: 0.0,
        });

        // Label do parâmetro centralizado verticalmente
        let label_col = rgba(palette::TEXT_PRIMARY[t], 200);
        let text_y = lane_top + (lane_h - 14.0) * 0.5;
        instances.extend(build_text_instances(&self.param_name, 4.0, text_y, label_col, 1.0));

        // Borda direita do gutter
        instances.push(RectInstance {
            x: gutter_w - 1.0,
            y: lane_top,
            w: 1.0,
            h: lane_h,
            color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 255],
            wave_offset: 0,
            wave_count: 0,
            clip_w: 0.0,
            clip_h: 0.0,
            radius: 0.0,
        });

        instances
    }
}

/// Converte AutomationCurve do core para pontos de view.
pub fn curve_to_points(curve: &AutomationCurve) -> Vec<AutomationPointView> {
    curve.points.iter().map(|p| AutomationPointView {
        beat: p.beat,
        value: p.value,
        interpolation: p.interpolation,
    }).collect()
}