use super::*;

impl App {
    pub fn handle_mouse_button(&mut self, state: ButtonState, pos: Option<(f32, f32)>) {
        match state {
            ButtonState::Pressed => {
                self.panning = true;
                if let Some(p) = pos {
                    self.last_cursor = p;
                }
            }
            ButtonState::Released => {
                // Se houver preview de Ã¡udio pendente, insere ao liberar o botÃ£o.
                if let Some(preview) = self.drop_preview.take() {
                    if let Some(path) = self.drop_path.take() {
                        self.load_audio_file(&path, preview.beat, preview.track);
                        // Inicia playback a partir do ponto onde o Ã¡udio foi solto.
                        self.start_playback_at(preview.beat);
                    }
                }
                self.panning = false;
            }
        }
    }

    /// Inicia o arrasto do loop na rÃ©gua a partir da posiÃ§Ã£o x do cursor.
    /// Retorna true se iniciou um arrasto de loop; false para comportamento padrÃ£o.
    pub fn begin_loop_drag(&mut self, mx: f32) -> bool {
        if !self.loop_enabled {
            return false;
        }
        let sx = self.beat_to_x(self.loop_start_beat);
        let ex = self.beat_to_x(self.loop_end_beat);
        let grab = 6.0;
        self.loop_drag_orig_start = self.loop_start_beat;
        self.loop_drag_orig_end = self.loop_end_beat;
        self.loop_drag_start_x = mx;
        if (mx - sx).abs() <= grab {
            self.loop_drag = Some(LoopDrag::Start);
        } else if (mx - ex).abs() <= grab {
            self.loop_drag = Some(LoopDrag::End);
        } else if mx >= sx && mx <= ex {
            self.loop_drag = Some(LoopDrag::Move);
        } else {
            return false;
        }
        true
    }

    /// Move a borda/seleÃ§Ã£o do loop com snap de 1 beat (arredonda o beat).
    pub fn handle_loop_drag(&mut self, mx: f32) {
        let Some(kind) = self.loop_drag else { return };
        let dx = mx - self.loop_drag_start_x;
        let delta_beats = (dx / self.px_per_beat.max(0.001)).round();
        match kind {
            LoopDrag::Start => {
                let new = (self.loop_drag_orig_start + delta_beats)
                    .max(0.0)
                    .min(self.loop_end_beat - 1.0);
                self.loop_start_beat = new.max(0.0);
            }
            LoopDrag::End => {
                let new = (self.loop_drag_orig_end + delta_beats).max(self.loop_start_beat + 1.0);
                self.loop_end_beat = new;
            }
            LoopDrag::Move => {
                let span = self.loop_drag_orig_end - self.loop_drag_orig_start;
                let new_start = (self.loop_drag_orig_start + delta_beats).max(0.0);
                self.loop_start_beat = new_start;
                self.loop_end_beat = new_start + span;
            }
        }
    }

    /// Inicia (Pressed) ou finaliza (Released) o *scrub* do playhead na rÃ©gua.
    /// Enquanto arrasta, `handle_cursor_moved` chama `set_playhead_from_x` em
    /// tempo real para seguir o cursor.
    pub fn handle_ruler_click(&mut self, state: ButtonState, pos: (f32, f32), _ruler_px: f32) {
        // ================================================================
        // MARKERS: clique na régua para criar/mover/selecionar marcadores
        // ================================================================
        let tool = current_tool();
        let marker_metrics = markers::MarkerMetrics {
            px_per_beat: self.px_per_beat,
            scroll_x: self.scroll_x,
            ruler_top: 0.0,
            ruler_height: self.ruler_px,
            theme: self.theme,
        };

        // Verifica se clicou em um marcador existente
        if let Some(hit_id) =
            markers::hit_test_marker(&self.marker_map.markers, pos, &marker_metrics)
        {
            match state {
                ButtonState::Pressed => {
                    self.marker_selected_id = Some(hit_id);
                    // Inicia drag se ferramenta Select
                    if tool == Tool::Select {
                        self.marker_inter.begin_drag(
                            &self.marker_map.markers,
                            pos,
                            &marker_metrics,
                        );
                    }
                    self.request_redraw();
                    return;
                }
                ButtonState::Released => {
                    if self.marker_inter.is_dragging() {
                        self.marker_inter.end_drag();
                        self.marker_commit();
                    }
                    self.request_redraw();
                    return;
                }
            }
        }

        // Clique vazio na régua: criar marcador (ferramenta Draw) ou deselecionar
        if matches!(tool, Tool::Draw | Tool::MarkerPosition)
            && matches!(state, ButtonState::Pressed)
        {
            let beat = ((pos.0 + self.scroll_x) / self.px_per_beat.max(0.001)).max(0.0);
            let snapped_beat = if self.snap_enabled {
                (beat * 4.0).round() / 4.0
            } else {
                beat
            };
            // Cria marcador de posição simples
            let id = self.marker_map.add_position(
                snapped_beat,
                format!("M{}", self.marker_map.markers.len() + 1),
            );
            self.marker_selected_id = Some(id);
            self.marker_write_back();
            self.request_redraw();
            return;
        }

        // Ferramenta MarkerRegion: cria região (início + fim)
        if tool == Tool::MarkerRegion && matches!(state, ButtonState::Pressed) {
            let beat = ((pos.0 + self.scroll_x) / self.px_per_beat.max(0.001)).max(0.0);
            let snapped_beat = if self.snap_enabled {
                (beat * 4.0).round() / 4.0
            } else {
                beat
            };
            let end_beat = snapped_beat + 4.0; // Região padrão de 4 beats
            let id = self.marker_map.add_region(
                snapped_beat,
                end_beat,
                format!("R{}", self.marker_map.markers.len() + 1),
            );
            self.marker_selected_id = Some(id);
            self.marker_write_back();
            self.request_redraw();
            return;
        }

        // Ferramenta MarkerLoop: cria loop (início + fim)
        if tool == Tool::MarkerLoop && matches!(state, ButtonState::Pressed) {
            let beat = ((pos.0 + self.scroll_x) / self.px_per_beat.max(0.001)).max(0.0);
            let snapped_beat = if self.snap_enabled {
                (beat * 4.0).round() / 4.0
            } else {
                beat
            };
            let end_beat = snapped_beat + 4.0; // Loop padrão de 4 beats
            let id = self.marker_map.add_loop(
                snapped_beat,
                end_beat,
                format!("L{}", self.marker_map.markers.len() + 1),
            );
            self.marker_selected_id = Some(id);
            self.marker_write_back();
            self.request_redraw();
            return;
        }

        // Ferramenta MarkerCdTrack: cria marcador CD
        if tool == Tool::MarkerCdTrack && matches!(state, ButtonState::Pressed) {
            let beat = ((pos.0 + self.scroll_x) / self.px_per_beat.max(0.001)).max(0.0);
            let snapped_beat = if self.snap_enabled {
                (beat * 4.0).round() / 4.0
            } else {
                beat
            };
            let track_num = (self
                .marker_map
                .markers
                .iter()
                .filter(|m| m.marker_type == daw_core::timeline::MarkerType::CDTrack)
                .count()
                + 1) as u32;
            let id =
                self.marker_map
                    .add_cd_track(snapped_beat, track_num, format!("CD{}", track_num));
            self.marker_selected_id = Some(id);
            self.marker_write_back();
            self.request_redraw();
            return;
        }

        // Clique vazio na régua com Select: deseleciona o marcador E inicia
        // o scrub da playhead. Sem ligar `scrubbing` aqui o arrasto na régua
        // não move a agulha: o `handle_cursor_moved` só segue o mouse quando
        // `scrubbing` está ativo (era o bug do "playhead não cola no mouse").
        if tool == Tool::Select && matches!(state, ButtonState::Pressed) {
            self.marker_selected_id = None;
            self.scrubbing = true;
            self.set_playhead_from_x(pos.0);
            self.request_redraw();
            return;
        }

        // Ferramenta Erase: apaga marcador clicado
        if tool == Tool::Erase && matches!(state, ButtonState::Pressed) {
            if let Some(hit_id) =
                markers::hit_test_marker(&self.marker_map.markers, pos, &marker_metrics)
            {
                self.marker_map.markers.retain(|m| m.id != hit_id);
                self.marker_selected_id = None;
                self.marker_write_back();
                self.request_redraw();
                return;
            }
        }

        // ================================================================
        // TIME WARP (Cubase): Alt + arrastar na régua
        //
        // Em vez de mover a playhead, move a POSIÇÃO MUSICAL: a linha de
        // compasso/batida agarrada no ponto inicial passa a soar onde o
        // usuário soltar. O mapa de tempo ganha a rampa que faz isso
        // (`TempoMap::warp_beat_to_target`) e o COMMIT grava no documento,
        // com undo — é o gesto que existe para encaixar a grade num evento
        // do áudio.
        //
        // O ponto agarrado vem do grid visual (o usuário pega uma linha); o
        // ALVO não é arredondado, porque o evento do áudio está onde está.
        // ================================================================
        if self.alt_pressed {
            match state {
                ButtonState::Pressed => {
                    self.timewarp_drag = Some(timeline_warp_beat(
                        pos.0,
                        self.scroll_x,
                        self.px_per_beat,
                        self.grid_beat_step(),
                    ));
                    self.request_redraw();
                }
                ButtonState::Released => {
                    if let Some(musical) = self.timewarp_drag.take() {
                        let target = timeline_beat_at_x(pos.0, self.scroll_x, self.px_per_beat);

                        if self.tempo_map.warp_beat_to_target(musical, target) {
                            self.tw_commit();
                        } else {
                            // Recusado (BPM fora da faixa, ou alvo antes do
                            // ponto de tempo anterior): documento intacto.
                            eprintln!("[timewarp] gesto recusado: beat {musical} -> alvo {target}");
                            self.request_redraw();
                        }
                    }
                }
            }

            return;
        }

        match state {
            ButtonState::Pressed => {
                self.scrubbing = true;
                self.set_playhead_from_x(pos.0);
            }
            ButtonState::Released => {
                self.scrubbing = false;
                // Um gesto de Time Warp que perdeu o ALT no meio (o key event
                // atualiza o modificador) não pode ficar pendurado: se o
                // release caiu aqui, o warp foi cancelado.
                self.timewarp_drag = None;
            }
        }
    }
}
