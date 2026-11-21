use super::*;

impl App {
    pub fn handle_primary_button(&mut self, state: ButtonState, pos: (f32, f32)) {
        // ====== Piano roll: Released sempre finaliza drags de loop/scrub ======
        // (coloquei ANTES dos "early returns de arrasto ativo" porque, se um
        //  drag estiver ativo, Released tem que encerrÃ¡-lo SEMPRE.)
        if self.show_piano_roll && state == ButtonState::Released {
            if self.pr_loop_drag.is_some() {
                self.pr_loop_drag = None;
            }
            if self.pr_scrubbing {
                self.pr_handle_ruler_click(state, pos);
            }
        }
        if self.show_timewarp && state == ButtonState::Released {
            if self.tw_scrubbing {
                self.tw_handle_ruler_click(state, pos);
            }
        }

        // Intercepta arrasto da splitter da área inferior (PR ou timewarp)
        if (self.show_piano_roll || self.show_timewarp) && self.splitter_drag {
            if state == ButtonState::Released {
                self.end_splitter_drag();
            } else {
                self.splitter_drag_move(pos.1);
            }
            return;
        }

        // Piano roll: durante arrasto de loop ativo (Pressed) â€” nada a fazer,
        // Released jÃ¡ foi tratado acima. Para garantir que nÃ£o re-ativamos por
        // engano, damos return quando hÃ¡ um loop drag ativo (sÃ³ importa no
        // Pressed, mas no Released o bloco acima jÃ¡ o zerou).
        if self.show_piano_roll && self.pr_loop_drag.is_some() {
            return;
        }

        // Intercepta clique no SCRUBBING do piano roll (sÃ³ durante arrasto ativo / Pressed)
        if self.show_piano_roll && self.pr_scrubbing && state == ButtonState::Pressed {
            // Durante scrubbing + Pressed: realinha a playhead (redundante com
            // cursor_moved, mas garante boa UX em clicks rÃ¡pidos).
            self.pr_handle_ruler_click(state, pos);
            return;
        }

        // Intercepta clique dentro da Ã¡rea do piano roll ou splitter (quando visÃ­vel)
        if self.show_piano_roll {
            let height = self.size.1 as f32;
            let width = self.size.0 as f32;
            let layout = self.layout(width, height);
            // Clique na splitter
            if self.splitter_hit_test(pos.1, height) {
                self.splitter_drag = true;
                return;
            }
            // Clique na RÃ‰GUA do piano roll (loop drag + scrub) â€” SEPARADO do conteÃºdo
            if let Some(pr_ruler) = &layout.piano_ruler {
                let ruler_top = pr_ruler.y;
                let ruler_bottom = pr_ruler.y + pr_ruler.height;
                if pos.1 >= ruler_top && pos.1 < ruler_bottom {
                    // Na rÃ©gua: PRIMEIRO tenta loop drag (apenas alÃ§as 6px,
                    // nÃ£o o centro). Se falhar â†’ scrub/playhead.
                    match state {
                        ButtonState::Pressed => {
                            if !self.pr_begin_loop_drag(pos.0) {
                                self.pr_handle_ruler_click(state, pos);
                            }
                        }
                        ButtonState::Released => {
                            // Released jÃ¡ foi tratado no topo da funÃ§Ã£o (fecha
                            // scrub/loop), mas garantimos aqui que, caso o
                            // usuÃ¡rio tenha feito um clique rÃ¡pido (Pressed +
                            // Released sem mover), o pr_handle_ruler_click seja
                            // chamado no Released caso o scrub ainda nÃ£o existia
                            // (Pressed caiu aqui via begin_loop_drag=false e
                            // setou pr_scrubbing = true).
                            // Esse bloco Ã© safety-net â€” o bloco do topo jÃ¡
                            // cobre o caso geral.
                            self.pr_handle_ruler_click(state, pos);
                        }
                    }
                    return;
                }
            }
            // Clique no CONTEÃšDO do piano roll (abaixo da rÃ©gua)
            if let Some(pr_ruler) = &layout.piano_ruler {
                if pos.1 >= pr_ruler.y + pr_ruler.height {
                    self.handle_piano_roll_click(state, pos);
                    return;
                }
            }
        }

        // Intercepta clique dentro da área do TIMEWARP ou splitter (visível)
        if self.show_timewarp {
            let height = self.size.1 as f32;
            let width = self.size.0 as f32;
            let layout = self.layout(width, height);
            if self.splitter_hit_test(pos.1, height) {
                self.splitter_drag = true;
                return;
            }
            // Clique na régua do timewarp (scrub da playhead própria)
            if let Some(tw_ruler) = &layout.timewarp_ruler {
                if pos.1 >= tw_ruler.y && pos.1 < tw_ruler.y + tw_ruler.height {
                    self.tw_handle_ruler_click(state, pos);
                    return;
                }
            }
            // Clique no CONTEÚDO do timewarp (abaixo da régua)
            if let Some(tw) = &layout.timewarp {
                if pos.1 >= tw.y {
                    self.handle_timewarp_click(state, pos);
                    return;
                }
            }
        }

        self.clip_interaction.sync_selection(self.clips.len());
        let cm = self.clip_metrics();
        let tool = current_tool();

        match state {
            ButtonState::Pressed => {
                if pos.1 < self.ruler_px {
                    // Régua → loop drag ou scrub (funciona em qualquer ferramenta).
                    //
                    // Com ALT a régua é a ferramenta TIME WARP: o gesto não
                    // pode ser capturado pelo arrasto da região de loop, senão
                    // quem tem loop ligado nunca consegue warpar o tempo.
                    if self.alt_pressed {
                        self.handle_ruler_click(state, pos, self.ruler_px);
                    } else if !self.begin_loop_drag(pos.0) {
                        self.handle_ruler_click(state, pos, self.ruler_px);
                    }
                } else {
                    let tempo_track_h = if self.show_tempo_track {
                        self.tempo_track_height
                    } else {
                        0.0
                    };
                    let tempo_track_top = self.ruler_px - self.scroll_y;
                    let tempo_track_bottom = tempo_track_top + tempo_track_h;

                    // ===== TEMPO TRACK INTERACTION =====
                    if self.show_tempo_track
                        && pos.1 >= tempo_track_top
                        && pos.1 < tempo_track_bottom
                    {
                        self.handle_tempo_track_click(state, pos);
                        return;
                    }

                    // InteraÃ§Ã£o com bolinhas de fade (in/out) e volume/ganho
                    if let Some((clip_idx, _kind)) = self.fade_interaction.hovered {
                        if let Some(clip) = self.clips.get(clip_idx) {
                            let (cx, cy, cw, ch) = clips::clip_rect(clip, &cm);
                            let header_h = 16.0;
                            let body_x = cx;
                            let body_y = cy + header_h;
                            let body_w = cw;
                            let body_h = ch - header_h;
                            if body_h > 0.0 && body_w > 0.0 {
                                if self.fade_interaction.begin_drag(
                                    pos,
                                    &self.fades,
                                    self.px_per_beat,
                                    body_x,
                                    body_y,
                                    body_w,
                                    body_h,
                                ) {
                                    return;
                                }
                            }
                        }
                    }
                    // Seam hover â†’ un-glue (split) â€” funciona em qualquer ferramenta
                    if let Some(seam_idx) = self.clip_interaction.seam_hover_idx {
                        if self.clip_interaction.is_seam_hovered() {
                            self.record_undo();
                            self.clip_interaction.unglue_seam(&mut self.clips, seam_idx);
                            self.rebuild_wave_data();
                            return;
                        }
                    }
                    // Duplo clique num clip ABRE o editor (piano roll / sampler)
                    // e cancela o que a ferramenta faria nesse press. Precisa vir
                    // antes do `match tool`: com a Draw, o primeiro clique já
                    // teria begun_draw e o segundo viraria um clip novo.
                    if let Some(idx) = self.clip_interaction.hit_test(&self.clips, pos, &cm) {
                        if self.detect_double_click(pos, idx) {
                            self.clip_interaction.select_only(idx);
                            let id = self.clips[idx].id;
                            self.emit_clip_open_request(id);
                            return;
                        }
                    } else {
                        self.last_press = None;
                    }
                    match tool {
                        Tool::Select => {
                            // Insere Ã¡udio arrastado se houver preview pendente.
                            if let Some(preview) = self.drop_preview.take() {
                                if let Some(path) = self.drop_path.take() {
                                    self.load_audio_file(&path, preview.beat, preview.track);
                                    // Inicia playback a partir do ponto onde o Ã¡udio foi solto.
                                    self.start_playback_at(preview.beat);
                                }
                            } else {
                                // comportamento padrÃ£o jÃ¡ existente
                                if let Some(idx) =
                                    self.clip_interaction.hit_test(&self.clips, pos, &cm)
                                {
                                    // multiseleÃ§Ã£o (arrasta todos selecionados). SenÃ£o,
                                    // colapsa para o grupo de glue.
                                    let already_selected = self
                                        .clip_interaction
                                        .selected
                                        .get(idx)
                                        .copied()
                                        .unwrap_or(false);
                                    self.clip_interaction.begin_drag(&self.clips, pos, &cm);
                                    if !already_selected {
                                        let chain =
                                            clips::ClipInteraction::find_group(&self.clips, idx);
                                        self.clip_interaction.select_chain(&self.clips, &chain);
                                    }
                                    self.edge_scroll_x = 0.0;
                                    self.edge_scroll_y = 0.0;
                                    self.edge_scroll_dir = 0;
                                } else {
                                    self.clip_interaction.begin_marquee(pos);
                                }
                            }
                        }
                        Tool::Draw => {
                            if pos.1 >= self.content_top() {
                                let cm = self.clip_metrics();
                                self.clip_interaction.begin_draw(pos, &cm);
                            }
                        }
                        Tool::Stretch => {
                            // Tenta iniciar stretch na borda direita; senÃ£o, seleciona
                            if !self.tool_stretch_start(pos) {
                                if let Some(idx) =
                                    self.clip_interaction.hit_test(&self.clips, pos, &cm)
                                {
                                    self.clip_interaction.select_only(idx);
                                } else {
                                    self.clip_interaction.begin_marquee(pos);
                                }
                            }
                        }
                        Tool::Slice => {
                            // Corta clip no ponto do clique
                            self.tool_slice(pos);
                        }
                        Tool::Erase => {
                            // Apaga clip clicado
                            self.tool_erase(pos);
                        }
                        Tool::Glue => {
                            // Cola clips adjacentes
                            self.tool_glue(pos);
                        }
                        Tool::MarkerPosition
                        | Tool::MarkerRegion
                        | Tool::MarkerLoop
                        | Tool::MarkerCdTrack => {
                            // Marcadores são tratados em handle_ruler_click
                        }
                    }
                }
            }
            ButtonState::Released => {
                if self.fade_interaction.is_dragging() {
                    // Persiste o fade/ganho no documento (por id de clip).
                    // O host grava em `Project::set_clip_fades` /
                    // `set_clip_gain`; sem isso o próximo rebuild do espelho
                    // (que agora re-sincroniza `fades` a partir do Project)
                    // apagaria o que o usuário acabou de arrastar.
                    if let Some(idx) = self
                        .fade_interaction
                        .active_drag
                        .as_ref()
                        .map(|d| d.clip_index)
                    {
                        let id = self.clips.get(idx).map(|c| c.id).unwrap_or(0);
                        let commit = self
                            .fades
                            .get(idx)
                            .map(|f| (id, f.in_len, f.out_len, f.gain_db));
                        if let Some((id, fi, fo, gain)) = commit {
                            if id != 0 {
                                if let Some(write) = self.fade_write_back.as_mut() {
                                    write(id, fi, fo, gain);
                                }
                            }
                        }
                    }
                    self.fade_interaction.end_drag();
                    return;
                }
                if self.loop_drag.is_some() {
                    self.loop_drag = None;
                    // Região de loop editada na régua → Transport compartilhado
                    // (motor de áudio + espelho do host usam a MESMA faixa).
                    self.emit_loop_write_back();
                } else if self.clip_interaction.is_dragging() {
                    let snap = self.snap_enabled;
                    let cm = self.clip_metrics();
                    self.clip_interaction.drag_move(pos, &cm, snap);
                    // Registra undo apenas se houve movimento real
                    if self.clip_interaction.did_drag_move() {
                        self.record_undo();
                    }
                    self.clip_interaction.end_drag(&mut self.clips, &cm, snap);
                    // Enganche o write-back estruturado de clips (hosted).
                    self.emit_clip_write_back();
                    // Reseta auto-scroll ao finalizar o arrasto.
                    self.edge_scroll_x = 0.0;
                    self.edge_scroll_y = 0.0;
                    self.edge_scroll_dir = 0;
                } else if self.clip_interaction.is_drawing() {
                    let cm = self.clip_metrics();
                    self.record_undo();
                    self.clip_interaction.end_draw(&mut self.clips, &cm);
                    self.clip_interaction.sync_selection(self.clips.len());
                    self.rebuild_wave_data();
                    self.emit_clip_write_back();
                } else if self.clip_interaction.is_marqueeing() {
                    self.clip_interaction.marquee_move(pos);
                    let cm = self.clip_metrics();
                    self.clip_interaction.end_marquee(&self.clips, &cm);
                } else if self.scrubbing || self.timewarp_drag.is_some() {
                    // O release do scrub E o do gesto de Time Warp terminam
                    // aqui: no warp o `scrubbing` nunca foi ligado (o gesto
                    // não move a playhead), então a condição inclui o
                    // `timewarp_drag` — sem isso o commit nunca aconteceria.
                    self.handle_ruler_click(state, pos, self.ruler_px);
                }
            }
        }
    }

    pub fn handle_cursor_moved(&mut self, pos: (f32, f32)) {
        // ====== CRÍTICO (winit 0.30): atualiza last_cursor ANTES de qualquer
        // lógica. O evento MouseInput (clique) NÃO traz a posição — ele usa
        // `app.last_cursor` para saber onde o clique foi. O CursorMoved
        // atualiza last_cursor; garanta que ele rode antes do primeiro clique.
        self.last_cursor = pos;

        // Se a splitter estÃ¡ sendo arrastada, redimensiona o piano roll em tempo real
        if self.splitter_drag {
            self.splitter_drag_move(pos.1);
            return;
        }

        // ====== Piano roll: loop drag / scrubbing (INDEPENDENTES da timeline) ======
        if self.show_piano_roll {
            if self.pr_loop_drag.is_some() {
                self.pr_handle_loop_drag(pos.0);
            } else if self.pr_scrubbing {
                self.pr_set_playhead_from_x(pos.0);
            }
        }

        // ====== Timewarp: scrub na régua / arrasto de marcadores de warp ======
        if self.show_timewarp {
            if self.tw_scrubbing {
                self.tw_set_scrub_beat(pos.0);
                self.request_redraw();
            } else if self.timewarp_inter.warp_is_dragging() {
                let m = self.timewarp_metrics();
                let snap = self.snap_enabled && !self.alt_pressed;
                let len = self.tw_warp_clip_data().map(|c| c.len).unwrap_or(0.0);
                if len > 0.0 {
                    self.timewarp_inter.warp_drag_move(pos, &m, snap, len);
                    self.tw_warp_preview_marker();
                }
                self.request_redraw();
            } else if self.timewarp_inter.is_dragging() {
                let m = self.timewarp_metrics();
                // Segurar Alt desliga o snap (beat/BPM livre).
                self.timewarp_inter.drag_move(pos, &m, !self.alt_pressed);
                self.request_redraw();
            } else if self.tw_warp_clip().is_some() {
                let m = self.timewarp_metrics();
                if let Some(clip) = self.tw_warp_clip_data() {
                    let markers = clip.warp_markers.clone();
                    let beat = clip.beat;
                    self.timewarp_inter
                        .warp_update_hover(&markers, beat, pos, &m);
                }
            } else {
                let m = self.timewarp_metrics();
                let points = self.tw_points();
                self.timewarp_inter.update_hover(&points, pos, &m);
            }
        }

        // ====== Tempo Track: hover / drag de pontos de tempo ======
        if self.show_tempo_track {
            let tempo_track_h = self.tempo_track_height;
            let tempo_track_top = self.ruler_px - self.scroll_y;
            let tempo_track_bottom = tempo_track_top + tempo_track_h;

            if pos.1 >= tempo_track_top && pos.1 < tempo_track_bottom {
                if self.tempo_track_inter.is_dragging() {
                    let m = self.tempo_track_metrics();
                    // Segurar Alt desliga o snap (beat/BPM livre).
                    self.tempo_track_inter.drag_move(pos, &m, !self.alt_pressed);
                    self.request_redraw();
                } else {
                    let m = self.tempo_track_metrics();
                    let points = self.tt_points();
                    self.tempo_track_inter.update_hover(&points, pos, &m);
                }
            }
        }

        // ====== Automation Lanes: hover / drag de pontos de automação ======
        if self.show_automation_lanes {
            // TODO: Integrar com tracks reais do Project no host
            // Por enquanto, não há automação real na versão demo
        }

        // ====== Markers: hover / drag de marcadores na régua ======
        let marker_metrics = markers::MarkerMetrics {
            px_per_beat: self.px_per_beat,
            scroll_x: self.scroll_x,
            ruler_top: 0.0,
            ruler_height: self.ruler_px,
            theme: self.theme,
        };
        self.marker_inter.update_hover(
            &self.marker_map.markers,
            pos,
            &marker_metrics,
            self.size.0 as f32,
        );
        self.marker_hovered_id = self.marker_inter.hovered_id;

        // Se está arrastando um marcador, move em tempo real
        if self.marker_inter.is_dragging() {
            self.marker_inter.drag_move(
                pos,
                &marker_metrics,
                self.snap_enabled && !self.alt_pressed,
                &mut self.marker_map.markers,
            );
            self.marker_write_back();
            self.request_redraw();
        }

        let cm = self.clip_metrics();
        if self.fade_interaction.is_dragging() {
            if let Some(ref drag) = self.fade_interaction.active_drag {
                // MantÃ©m o cursor de override enquanto arrasta o handle.
                match drag.kind {
                    fades::FadeHandleKind::GainLevel => set_gain_override(true),
                    _ => set_hand_override(true),
                }
                if let Some(clip) = self.clips.get(drag.clip_index) {
                    let (_, _, cw, ch) = clips::clip_rect(clip, &cm);
                    let body_w = cw - 2.0;
                    let body_h = ch - 16.0;
                    self.fade_interaction.update_drag(
                        pos,
                        &mut self.fades,
                        self.px_per_beat,
                        clip.len,
                        body_w,
                        body_h,
                    );
                    self.rebuild_wave_data();
                    // Se jÃ¡ estÃ¡ tocando, sincroniza ganho em tempo real.
                    #[cfg(feature = "audio")]
                    if self.playing {
                        self.sync_audio_engine();
                    }
                }
            }
        } else if self.loop_drag.is_some() {
            self.handle_loop_drag(pos.0);
        } else if self.clip_interaction.is_dragging() {
            let snap = self.snap_enabled;
            self.clip_interaction.drag_move(pos, &cm, snap);
            // Edge scroll Ã© tratado no RedrawRequested (roda todo frame)
        } else if self.clip_interaction.is_drawing() {
            self.clip_interaction.draw_move(pos, &cm);
        } else if self.clip_interaction.is_marqueeing() {
            self.clip_interaction.marquee_move(pos);
        } else if self.scrubbing {
            self.set_playhead_from_x(pos.0);
        }
        self.clip_interaction.update_hover(&self.clips, pos, &cm);

        // Atualiza detecÃ§Ã£o de hover sobre bolinhas de fade e ganho
        if !self.clip_interaction.is_dragging() && !self.fade_interaction.is_dragging() {
            self.fade_interaction.hovered = None;
            for (i, clip) in self.clips.iter().enumerate() {
                if !clip.is_midi {
                    if let Some(fade) = self.fades.get(i) {
                        let (cx, cy, cw, ch) = clips::clip_rect(clip, &cm);
                        let header_h = 16.0;
                        let body_y = cy + header_h;
                        let body_h = ch - header_h;
                        if let Some(kind) = fades::hit_test_clip_handles(
                            i,
                            fade,
                            cx,
                            body_y,
                            cw,
                            body_h,
                            self.px_per_beat,
                            pos,
                        ) {
                            self.fade_interaction.hovered = Some((i, kind));
                            break;
                        }
                    }
                }
            }
        }

        // (Removido) O 'snap magnético' que teleportava o cursor do SO até o
        // handle de fade/ganho: ele roubava o arrasto do clip e movia o
        // ponteiro sozinho quando passava perto do knob no topo do clip.

        // - Enquanto arrasta um handle, forÃ§a o cursor apropriado (hand ou gain).
        // - Quando nÃ£o estÃ¡ arrastando, usa hover para escolher o cursor.
        // - Caso contrÃ¡rio, limpa quaisquer sobrescriÃ§Ãµes de cursor.
        // Primeiro limpa ambos para garantir estado consistente.
        set_gain_override(false);
        set_hand_override(false);
        let mut hand_or_gain_active = false;

        if self.fade_interaction.is_dragging() {
            if let Some(ref drag) = self.fade_interaction.active_drag {
                match drag.kind {
                    fades::FadeHandleKind::GainLevel => {
                        set_gain_override(true);
                        hand_or_gain_active = true;
                    }
                    _ => {
                        set_hand_override(true);
                        hand_or_gain_active = true;
                    }
                }
            }
        } else if let Some((_, kind)) = self.fade_interaction.hovered {
            if kind == fades::FadeHandleKind::GainLevel {
                set_gain_override(true);
                hand_or_gain_active = true;
            } else {
                set_hand_override(true);
                hand_or_gain_active = true;
            }
        } else if self.clip_interaction.is_seam_hovered() {
            set_cursor_override(Tool::Slice);
            hand_or_gain_active = true; // cursor override ativo
        }

        // Cursor de resize vertical sobre a splitter do piano roll
        // Só limpa o vresize se NÃO tivermos hand/gain ativo (evita sobrescrever).
        if self.splitter_drag || self.splitter_hit_test(self.last_cursor.1, self.size.1 as f32) {
            set_vresize_override(true);
        } else if !hand_or_gain_active {
            set_vresize_override(false);
        }

        if self.panning
            && !self.scrubbing
            && !self.clip_interaction.is_dragging()
            && !self.clip_interaction.is_drawing()
            && !self.clip_interaction.is_stretching()
        {
            let (lx, ly) = self.last_cursor;
            self.scroll_x = (self.scroll_x - (pos.0 - lx)).max(0.0);
            self.scroll_y = (self.scroll_y - (pos.1 - ly)).max(0.0);
            self.clamp_scroll_y(self.input_view_size().1);
        }
        // Atualiza a posiÃ§Ã£o do preview de Ã¡udio arrastado
        if self.drop_preview.is_some() {
            let (beat, track) = self.cursor_to_snapped_beat_track(pos);
            if let Some(preview) = self.drop_preview.as_mut() {
                preview.beat = beat;
                preview.track = track;
            }
        }

        let mut constrained = pos;
        if self.fade_interaction.is_dragging() {
            if let Some(ref drag) = self.fade_interaction.active_drag {
                match drag.axis_lock {
                    Some(fades::DragAxisLock::Horizontal) => {
                        // fixa a coordenada Y ao inÃ­cio do arrasto
                        constrained.1 = drag.start_cursor.1;
                    }
                    Some(fades::DragAxisLock::Vertical) => {
                        // fixa a coordenada X ao inÃ­cio do arrasto
                        constrained.0 = drag.start_cursor.0;
                    }
                    None => {}
                }
            }
        }
        self.last_cursor = constrained;

        // O cursor fÃ­sico NÃƒO Ã© repospicionado durante o arrasto (trava de eixo).
        // O lock jÃ¡ Ã© aplicado em `update_drag` (dx/dy = 0) e aqui via `constrained`
        // para o cÃ¡lculo de panning. Warpar o cursor a cada frame causa jitter
        // por loop de feedback (warp â†’ CursorMoved â†’ warp â†’ ...).
    }

    /// Detecta quando o cursor estÃ¡ perto das bordas da janela durante um
    /// arrasto de clip e ajusta a velocidade de auto-scroll proporcional Ã
    /// proximidade (quanto mais perto da borda, mais rÃ¡pido rola).
    /// Zona de trigger: 40px de cada borda. Velocidade mÃ¡xima: 24px/frame.
    pub fn update_edge_scroll(&mut self, pos: (f32, f32)) {
        let (width, height) = self.input_view_size();
        let edge_zone = 40.0;
        let max_speed = 24.0;

        // Borda esquerda / direita (scroll horizontal)
        if pos.0 < edge_zone {
            // Quanto mais prÃ³ximo da borda, mais rÃ¡pido (0..1)
            let proximity = 1.0 - (pos.0 / edge_zone).clamp(0.0, 1.0);
            self.edge_scroll_x = proximity * max_speed;
            self.edge_scroll_dir = -1; // rola para esquerda (conteÃºdo entra da direita)
        } else if pos.0 > width - edge_zone {
            let proximity = 1.0 - ((width - pos.0) / edge_zone).clamp(0.0, 1.0);
            self.edge_scroll_x = proximity * max_speed;
            self.edge_scroll_dir = 1; // rola para direita
        } else {
            self.edge_scroll_x = 0.0;
        }

        // Borda top / bottom (scroll vertical)
        // SÃ³ rola verticalmente se o cursor estiver abaixo da rÃ©gua
        let body_top = self.content_top();
        if pos.1 < body_top + edge_zone {
            // Top edge: rola para cima
            let proximity_y = if pos.1 < body_top {
                // Cursor na rÃ©gua: proximidade mÃ¡xima
                1.0
            } else {
                1.0 - ((pos.1 - body_top) / edge_zone).clamp(0.0, 1.0)
            };
            self.edge_scroll_y = proximity_y * max_speed * 0.6; // velocidade Y menor
        } else if pos.1 > height - edge_zone {
            // Bottom edge: rola para baixo
            let proximity_y = 1.0 - ((height - pos.1) / edge_zone).clamp(0.0, 1.0);
            self.edge_scroll_y = proximity_y * max_speed * 0.6;
        } else {
            self.edge_scroll_y = 0.0;
        }
    }

    /// Aplica o auto-scroll calculado no frame atual, atualizando os offsets
    /// de scroll horizontal e vertical. Compensa a posiÃ§Ã£o do clip em arrasto
    /// (drag_live_beat) para que o overlay continue colado ao cursor mesmo
    /// apÃ³s a viewport rolar.
    pub fn apply_edge_scroll(&mut self) {
        if self.edge_scroll_x == 0.0 && self.edge_scroll_y == 0.0 {
            return;
        }
        let dx = self.edge_scroll_x * self.edge_scroll_dir as f32;
        let dy = self.edge_scroll_y;

        // Ajusta scroll horizontal
        let prev_sx = self.scroll_x;
        self.scroll_x = (self.scroll_x + dx).max(0.0);
        // Ajusta scroll vertical
        let prev_sy = self.scroll_y;
        self.scroll_y = (self.scroll_y + dy).max(0.0);
        self.clamp_scroll_y(self.input_view_size().1);

        // Compensa o beat virtual do clip em arrasto para que ele nÃ£o "salte"
        // na tela. O overlay Ã© renderizado em:
        //   x = live_beat * px_per_beat - scroll_x
        // Se aumentamos scroll_x em `dx`, precisamos aumentar live_beat em
        // `dx / px_per_beat` para manter a mesma posiÃ§Ã£o na tela.
        // Usa o delta efetivo (nÃ£o o pretendido) â€” se scroll_x foi clampado,
        // o clip nÃ£o deve ser empurrado alÃ©m do limite.
        //
        // CRÃTICO: `drag_move` recalcula live_beat/live_track a partir de
        // `drag_start_cursor` (âncora) a cada evento de move. Compensar apenas
        // `drag_live_beat` seria apagado no move seguinte (o clip "para" na
        // timeline enquanto a view continua rolando). Por isso o âncora também
        // é compensado em PX (na direção oposta ao scroll), fazendo o delta
        // acumular o scroll e o clip permanecer colado ao cursor.
        if self.clip_interaction.is_dragging() && !self.clip_interaction.is_stretching() {
            let pps = self.px_per_beat.max(0.001);
            let actual_dx = self.scroll_x - prev_sx;
            let actual_dy = self.scroll_y - prev_sy;
            if actual_dx != 0.0 {
                self.clip_interaction.drag_start_cursor.0 -= actual_dx;
                self.clip_interaction.drag_live_beat += actual_dx / pps;
            }
            if actual_dy != 0.0 {
                self.clip_interaction.drag_start_cursor.1 -= actual_dy;
            }
        }
        // Mesma compensação de âncora para os handles de fade/ganho: o
        // `update_drag` recalcula o delta a partir de `start_cursor` a cada
        // move, então o âncora precisa acompanhar o scroll para o knob
        // permanecer colado ao cursor mesmo com auto-scroll nas bordas.
        if self.fade_interaction.is_dragging() {
            if let Some(ref mut drag) = self.fade_interaction.active_drag {
                let actual_dx = self.scroll_x - prev_sx;
                let actual_dy = self.scroll_y - prev_sy;
                if actual_dx != 0.0 {
                    drag.start_cursor.0 -= actual_dx;
                }
                if actual_dy != 0.0 {
                    drag.start_cursor.1 -= actual_dy;
                }
            }
        }
    }

    /// Atualiza o pan (botão do meio) consultando a posição atual do mouse via Win32.
    /// Útil quando o SO não envia eventos CursorMoved durante o drag do botão do meio.
    pub fn update_panning_from_mouse(&mut self) {
        if let Some(ref win) = self.window {
            if let Some(pos) = crate::app::window::current_cursor_pos(win) {
                let (lx, ly) = self.last_cursor;
                self.scroll_x = (self.scroll_x - (pos.0 - lx)).max(0.0);
                self.scroll_y = (self.scroll_y - (pos.1 - ly)).max(0.0);
                self.clamp_scroll_y(self.input_view_size().1);
                self.last_cursor = pos;
                self.request_redraw();
            }
        }
    }
}
