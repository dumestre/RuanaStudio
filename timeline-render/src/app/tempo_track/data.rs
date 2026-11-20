//! Geometria da tempo track: faixa dedicada no topo da timeline (eixo Y = BPM).
//!
//! Diferente do timewarp (que é um editor em área expandida na parte inferior),
//! a tempo track é uma track fina no topo da timeline, sempre visível quando
//

use crate::{palette, rgba, RectInstance};

/// Altura fixa da tempo track (px lógicos).
pub const TEMPO_TRACK_HEIGHT: f32 = 60.0;

/// Faixa de BPM exibida (fixa, como o timewarp).
pub const BPM_MIN: f32 = 20.0;
pub const BPM_MAX: f32 = 300.0;

/// Um evento de tempo exibido. Espelha `daw_core::timeline::TempoChange`.
#[derive(Clone, Debug)]
pub struct TempoPoint {
    pub beat: f32,
    pub bpm: f32,
}

/// Conversões cursor ↔ conteúdo da tempo track.
#[derive(Clone)]
pub struct TempoTrackMetrics {
    pub px_per_beat: f32,
    pub scroll_x: f32,
    /// Topo da tempo track (coordenada Y global da janela).
    pub track_top: f32,
    /// Altura da tempo track.
    pub track_h: f32,
    pub theme: usize,
}

impl TempoTrackMetrics {
    /// Converte beat → coordenada de janela.
    pub fn beat_to_x(&self, beat: f32) -> f32 {
        beat * self.px_per_beat - self.scroll_x
    }

    /// Converte x de janela → beat.
    pub fn x_to_beat(&self, x: f32) -> f32 {
        (x + self.scroll_x) / self.px_per_beat.max(0.001)
    }

    /// Converte BPM → coordenada de janela (BPM alto no topo da track).
    pub fn bpm_to_y(&self, bpm: f32) -> f32 {
        let norm = ((BPM_MAX - bpm) / (BPM_MAX - BPM_MIN)).clamp(0.0, 1.0);
        self.track_top + norm * self.track_h
    }

    /// Converte y de janela → BPM (clampado à faixa exibida).
    pub fn y_to_bpm(&self, y: f32) -> f32 {
        let rel = ((y - self.track_top) / self.track_h.max(1.0)).clamp(0.0, 1.0);
        (BPM_MAX - rel * (BPM_MAX - BPM_MIN)).clamp(BPM_MIN, BPM_MAX)
    }

    /// Converte cursor em (beat, bpm).
    pub fn cursor_to_beat_bpm(&self, cursor: (f32, f32)) -> (f32, f32) {
        (self.x_to_beat(cursor.0).max(0.0), self.y_to_bpm(cursor.1))
    }

    /// Retorna as instâncias de fundo da tempo track (fundo + grid vertical).
    pub fn build_background_instances(&self, width: f32) -> Vec<RectInstance> {
        let t = self.theme.clamp(0, 6);
        let mut instances = Vec::new();

        let track_top = self.track_top;
        let track_h = self.track_h.max(0.0);
        let track_bottom = track_top + track_h;

        // Fundo base da track: TRANSPARENTE de propósito. O grid e as colunas
        // alternadas da timeline (desenhados abaixo da régua) atravessam a
        // faixa de tempo; só a curva de BPM e os marcadores ficam por cima.
        instances.push(RectInstance {
            x: 0.0,
            y: track_top,
            w: width,
            h: track_h,
            color: rgba(palette::TIMELINE_BG[t], 0),
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
                        x: xs, y: track_top, w: 1.0, h: track_h,
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
                        x: xs, y: track_top, w: 1.0, h: track_h,
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
                        x: xs, y: track_top, w: 1.0, h: track_h,
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
                        x: xs, y: track_top, w: 1.0, h: track_h,
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
                        x: xs, y: track_top, w: 1.0, h: track_h,
                        color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 150],
                        wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
                    });
                }
            }
        }

        // Linha separadora inferior da tempo track (fina, sem esconder o grid)
        instances.push(RectInstance {
            x: 0.0, y: track_bottom - 1.0, w: width, h: 1.0,
            color: [palette::BASE[t].0, palette::BASE[t].1, palette::BASE[t].2, 140],
            wave_offset: 0, wave_count: 0, clip_w: 0.0, clip_h: 0.0, radius: 0.0,
        });

        instances
    }

    /// Instâncias do gutter esquerdo com o label "TEMPO" (no lugar do nome da track).
    pub fn build_gutter_instances(&self) -> Vec<RectInstance> {
        use crate::build_text_instances;
        let t = self.theme.clamp(0, 6);
        let mut instances = Vec::new();
        let track_top = self.track_top;
        let track_h = self.track_h.max(0.0);

        // Fundo do gutter (coluna "TEMPO" onde o track header tem o nome).
        // Translúcido: o grid da timeline continua visível por baixo.
        let gutter_w = 60.0;
        instances.push(RectInstance {
            x: 0.0,
            y: track_top,
            w: gutter_w,
            h: track_h,
            color: rgba(palette::BASE[t], 90),
            wave_offset: 0,
            wave_count: 0,
            clip_w: 0.0,
            clip_h: 0.0,
            radius: 0.0,
        });

        // Label "TEMPO" centralizado verticalmente
        let label_col = rgba(palette::TEXT_PRIMARY[t], 200);
        let text_y = track_top + (track_h - 14.0) * 0.5;
        instances.extend(build_text_instances("TEMPO", 4.0, text_y, label_col, 1.0));

        // Borda direita do gutter
        instances.push(RectInstance {
            x: gutter_w - 1.0,
            y: track_top,
            w: 1.0,
            h: track_h,
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