use super::*;

// ===== Piano roll interaction helpers =====

impl App {
    pub(super) fn piano_roll_metrics(&self) -> crate::app::piano_roll::PianoRollMetrics {
        let height = self.size.1 as f32;
        let width = self.size.0 as f32;
        let layout = self.layout(width, height);
        let pr_ruler = layout.piano_ruler.unwrap();
        let pr_rect = layout.piano_roll.unwrap();

        let m = crate::app::piano_roll::PianoRollMetrics {
            px_per_beat: self.piano_roll_px_per_beat,
            scroll_x: self.piano_roll_scroll_x,
            scroll_y: self.piano_roll_scroll_y,
            ruler_px: pr_ruler.y + crate::app::piano_roll::PIANO_RULER_H,
            row_height: 6.0,
            theme: self.theme.clamp(0, 6),
            top_pitch: 96,
            bottom_pitch: 0,
            key_height: self.piano_roll_key_height,
            roll_height: pr_rect.height,
        };
        m
    }

    pub(super) fn handle_piano_roll_click(&mut self, state: ButtonState, pos: (f32, f32)) {
        let m = self.piano_roll_metrics();
        let tool = current_tool();

        match state {
            ButtonState::Pressed => {
                if self.splitter_hit_test(pos.1, self.size.1 as f32) {
                    self.splitter_drag = true;
                    return;
                }

                self.piano_roll_inter
                    .sync_selection(self.piano_roll_notes.len());

                match tool {
                    Tool::Select => {
                        if self.piano_roll_inter.is_dragging() {
                            self.piano_roll_inter.drag_move(pos, &m);
                        } else if !self
                            .piano_roll_inter
                            .begin_drag(&self.piano_roll_notes, pos, &m)
                        {
                            self.piano_roll_inter.begin_marquee(pos);
                        }
                    }
                    Tool::Draw => {
                        self.piano_roll_inter.begin_draw(pos, &m);
                    }
                    _ => {}
                }
            }
            ButtonState::Released => {
                if self.splitter_drag {
                    self.end_splitter_drag();
                    return;
                }

                match tool {
                    Tool::Select => {
                        if self.piano_roll_inter.is_dragging() {
                            self.piano_roll_inter
                                .end_drag(&mut self.piano_roll_notes, &m);
                            self.sync_piano_roll_to_core();
                            self.emit_piano_roll_write_back();
                        } else if self.piano_roll_inter.is_marqueeing() {
                            self.piano_roll_inter
                                .end_marquee(&self.piano_roll_notes, &m);
                        }
                    }
                    Tool::Draw => {
                        if self.piano_roll_inter.is_drawing() {
                            let created = self
                                .piano_roll_inter
                                .end_draw(&mut self.piano_roll_notes, &m);
                            for &idx in &created {
                                if let Some(note) = self.piano_roll_notes.get(idx) {
                                    self.note_editor.add(
                                        note.pitch,
                                        note.start,
                                        note.length,
                                        note.velocity,
                                    );
                                }
                            }
                            self.sync_piano_roll_from_core();
                            self.emit_piano_roll_write_back();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

// ============================================================================
// TIME WARP: geometria da régua
//
// Funções livres (não métodos de `App`) de propósito: são a única parte do
// gesto que dá para testar sem uma GPU, e é onde mora a conta que decide QUAL
// linha da grade o usuário agarrou.
// ============================================================================

/// Beat sob uma coordenada X da timeline (sem snap).
pub fn timeline_beat_at_x(x: f32, scroll_x: f32, px_per_beat: f32) -> f32 {
    ((x + scroll_x) / px_per_beat.max(0.001)).max(0.0)
}

/// Beat do grid visual mais próximo de X — a linha que o usuário "agarra" com
/// a ferramenta Time Warp.
///
/// `step` é o passo do grid naquele zoom (`App::grid_beat_step`), o mesmo que
/// o renderer desenha: o gesto pega a linha que está na tela.
pub fn timeline_warp_beat(x: f32, scroll_x: f32, px_per_beat: f32, step: f32) -> f32 {
    let beat = timeline_beat_at_x(x, scroll_x, px_per_beat);
    let step = if step.is_finite() && step > 0.0 {
        step
    } else {
        1.0
    };

    ((beat / step).round() * step).max(0.0)
}

#[cfg(test)]
mod timewarp_ruler_tests {
    use super::*;

    #[test]
    pub(super) fn beat_sob_a_coordenada() {
        // 40 px por beat, sem scroll: x=100 -> beat 2.5.
        assert!((timeline_beat_at_x(100.0, 0.0, 40.0) - 2.5).abs() < 1e-5);
        // Com scroll de 200 px, x=0 -> beat 5.
        assert!((timeline_beat_at_x(0.0, 200.0, 40.0) - 5.0).abs() < 1e-5);
        // Antes do inicio do conteudo clampa em 0.
        assert_eq!(timeline_beat_at_x(-500.0, 0.0, 40.0), 0.0);
        // px_per_beat degenerado nao divide por zero.
        assert!(timeline_beat_at_x(100.0, 0.0, 0.0).is_finite());
    }

    #[test]
    pub(super) fn ponto_agarrado_vem_do_grid_visual() {
        // Grid de 1 beat: x=100 (beat 2.5) -> a linha mais proxima e a 3.
        assert!((timeline_warp_beat(100.0, 0.0, 40.0, 1.0) - 3.0).abs() < 1e-5);
        // Grid de 1/4: 2.5 ja e uma linha do grid, fecha nela mesma.
        assert!((timeline_warp_beat(100.0, 0.0, 40.0, 0.25) - 2.5).abs() < 1e-5);
        // Grid de compasso (4 beats): 2.5 -> 4 (inicio do compasso seguinte).
        assert!((timeline_warp_beat(100.0, 0.0, 40.0, 4.0) - 4.0).abs() < 1e-5);
        // Step invalido nao pode zerar o gesto: cai em 1 beat.
        assert!((timeline_warp_beat(100.0, 0.0, 40.0, 0.0) - 3.0).abs() < 1e-5);
        assert!((timeline_warp_beat(100.0, 0.0, 40.0, f32::NAN) - 3.0).abs() < 1e-5);
    }
}
