use super::*;

impl App {
    pub(super) fn timewarp_metrics(&self) -> crate::app::timewarp::TimewarpMetrics {
        let height = self.size.1 as f32;
        let width = self.size.0 as f32;
        let layout = self.layout(width, height);
        let tw_ruler = layout.timewarp_ruler.unwrap();
        let tw_rect = layout.timewarp.unwrap();
        crate::app::timewarp::TimewarpMetrics {
            px_per_beat: self.timewarp_px_per_beat,
            scroll_x: self.timewarp_scroll_x,
            ruler_px: tw_ruler.y,
            ruler_h: tw_ruler.height,
            body_top: tw_rect.y,
            body_h: tw_rect.height,
            theme: self.theme.clamp(0, 6),
        }
    }

    /// Pontos de tempo atuais (espelho das mudanças do `TempoMap`).
    pub(super) fn tw_points(&self) -> Vec<crate::app::timewarp::TempoPoint> {
        self.tempo_map
            .changes
            .iter()
            .map(|c| crate::app::timewarp::TempoPoint {
                beat: c.beat,
                bpm: c.bpm,
            })
            .collect()
    }

    /// Aplica uma lista de pontos editada de volta ao `TempoMap` e escreve no
    /// projeto (via callback de write-back).
    pub(super) fn tw_apply_points(&mut self, points: &[crate::app::timewarp::TempoPoint]) {
        for p in points {
            self.tempo_map.set_change(p.beat, p.bpm);
        }
        self.tw_write_back();
        // Fim de gesto: é aqui que o documento recebe o mapa (undo + o evento
        // que faz o áudio re-sincronizar). O write-back acima é só o preview
        // ao vivo da engine.
        self.tw_commit();
    }

    /// Dispara o callback de write-back (host grava no projeto) e re-renderiza.
    pub(super) fn tw_write_back(&mut self) {
        if let Some(cb) = &self.tempo_write_back {
            cb(&self.tempo_map);
        }
        self.request_redraw();
    }

    /// Fim de um gesto de tempo: o host grava o mapa no documento (undo).
    pub(super) fn tw_commit(&mut self) {
        if let Some(cb) = &self.tempo_commit {
            cb(&self.tempo_map);
        }
        self.request_redraw();
    }

    /// Clique na régua do TIMEWARP: inicia/finaliza o scrub da playhead.
    pub fn tw_handle_ruler_click(&mut self, state: ButtonState, pos: (f32, f32)) {
        match state {
            ButtonState::Pressed => {
                self.tw_scrubbing = true;
                self.tw_set_scrub_beat(pos.0);
                self.request_redraw();
            }
            ButtonState::Released => {
                self.tw_scrubbing = false;
                self.request_redraw();
            }
        }
    }

    /// Clique no corpo do timewarp: edição de pontos de tempo
    /// (Select = mover, Draw = adicionar, Erase = remover) OU edição de
    /// marcadores de AudioWarp (duplo clique cria, arrasto estica).
    pub fn handle_timewarp_click(&mut self, state: ButtonState, pos: (f32, f32)) {
        let m = self.timewarp_metrics();
        let tool = current_tool();

        // ===== MODO WARP: clip de áudio alvo do editor de waveform =====
        if self.tw_warp_clip().is_some() {
            match state {
                ButtonState::Pressed => {
                    if self.splitter_hit_test(pos.1, self.size.1 as f32) {
                        self.splitter_drag = true;
                        return;
                    }
                    let (clip_beat, markers) = match self.tw_warp_clip_data() {
                        Some(c) => (c.beat, c.warp_markers.clone()),
                        None => return,
                    };
                    if self
                        .timewarp_inter
                        .warp_begin_drag(&markers, clip_beat, pos, &m)
                    {
                        self.request_redraw();
                        return;
                    }
                    // Não agarrou marcador: um duplo clique cria um novo.
                    if self.timewarp_inter.warp_register_press(pos.0) {
                        let offset = m.x_to_beat(pos.0) - clip_beat;
                        self.tw_warp_add_marker_at(clip_beat, offset);
                    }
                }
                ButtonState::Released => {
                    if self.splitter_drag {
                        self.end_splitter_drag();
                        return;
                    }
                    if let Some((idx, live)) = self.timewarp_inter.warp_end_drag() {
                        self.tw_warp_move_marker(idx, live);
                    }
                    self.timewarp_preview_marker = None;
                }
            }
            self.request_redraw();
            return;
        }

        // ===== MODO TEMPO/BPM (sem clip de áudio alvo) =====
        match state {
            ButtonState::Pressed => {
                if self.splitter_hit_test(pos.1, self.size.1 as f32) {
                    self.splitter_drag = true;
                    return;
                }

                match tool {
                    Tool::Select => {
                        let points = self.tw_points();
                        let _ = self.timewarp_inter.begin_drag(&points, pos, &m);
                    }
                    Tool::Draw => {
                        let (mut beat, bpm) = m.cursor_to_beat_bpm(pos);
                        if !self.alt_pressed {
                            beat = beat.round();
                        }
                        self.tempo_map.set_change(beat, bpm);
                        self.tw_write_back();
                        return;
                    }
                    Tool::Erase => {
                        let points = self.tw_points();
                        if let Some(idx) = self.timewarp_inter.hit_test(&points, pos, &m) {
                            if let Some(p) = points.get(idx) {
                                self.tempo_map.remove_change(p.beat);
                                self.tw_write_back();
                                return;
                            }
                        }
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
                        if self.timewarp_inter.is_dragging() {
                            let mut points = self.tw_points();
                            self.timewarp_inter.end_drag(&mut points);
                            self.tw_apply_points(&points);
                        }
                    }
                    _ => {}
                }
            }
        }
        self.request_redraw();
    }

    /// Handle click/interaction on the tempo track (top track showing BPM curve).
    pub fn handle_tempo_track_click(&mut self, state: ButtonState, pos: (f32, f32)) {
        let m = self.tempo_track_metrics();
        let tool = current_tool();

        match state {
            ButtonState::Pressed => {
                match tool {
                    Tool::Select => {
                        let points = self.tt_points();
                        let _ = self.tempo_track_inter.begin_drag(&points, pos, &m);
                    }
                    Tool::Draw => {
                        let (beat, bpm) = m.cursor_to_beat_bpm(pos);
                        // Snap to beat grid by default, free placement with Alt
                        let beat = if self.alt_pressed { beat } else { beat.round() };
                        self.tempo_map.set_change(beat, bpm);
                        self.tt_write_back();
                        return;
                    }
                    Tool::Erase => {
                        let points = self.tt_points();
                        if let Some(idx) = self.tempo_track_inter.hit_test(&points, pos, &m) {
                            if let Some(p) = points.get(idx) {
                                self.tempo_map.remove_change(p.beat);
                                self.tt_write_back();
                                return;
                            }
                        }
                    }
                    _ => {}
                }
            }
            ButtonState::Released => match tool {
                Tool::Select => {
                    if self.tempo_track_inter.is_dragging() {
                        let mut points = self.tt_points();
                        self.tempo_track_inter.end_drag(&mut points);
                        self.tt_apply_points(&points);
                    }
                }
                _ => {}
            },
        }
        self.request_redraw();
    }

    /// Build metrics for tempo track interaction.
    pub(super) fn tempo_track_metrics(&self) -> crate::app::tempo_track::TempoTrackMetrics {
        crate::app::tempo_track::TempoTrackMetrics {
            px_per_beat: self.px_per_beat,
            scroll_x: self.scroll_x,
            track_top: self.ruler_px - self.scroll_y,
            track_h: if self.show_tempo_track {
                self.tempo_track_height
            } else {
                0.0
            },
            theme: self.theme,
        }
    }

    /// Get tempo map points as TempoPoint for tempo track.
    pub(super) fn tt_points(&self) -> Vec<crate::app::tempo_track::TempoPoint> {
        self.tempo_map
            .changes
            .iter()
            .map(|c| crate::app::tempo_track::TempoPoint {
                beat: c.beat,
                bpm: c.bpm,
            })
            .collect()
    }

    /// Write back tempo map changes (live preview during drag).
    pub(super) fn tt_write_back(&mut self) {
        if let Some(f) = &mut self.tempo_track_write_back {
            f(&self.tempo_map);
        }
    }

    /// Apply tempo track points after drag commit (records undo).
    pub(super) fn tt_apply_points(&mut self, points: &[crate::app::tempo_track::TempoPoint]) {
        let mut new_map = self.tempo_map.clone();
        new_map.changes.clear();
        for p in points {
            new_map.set_change(p.beat, p.bpm);
        }
        self.tempo_map = new_map;
        if let Some(f) = &self.tempo_track_commit {
            f(&self.tempo_map);
        }
    }

    /// Write back marker map changes (live preview during drag).
    pub(super) fn marker_write_back(&mut self) {
        if let Some(f) = &mut self.marker_write_back {
            f(&self.marker_map);
        }
    }

    /// Apply marker map changes after drag commit (records undo).
    pub(super) fn marker_commit(&mut self) {
        if let Some(f) = &mut self.marker_commit {
            f(&self.marker_map);
        }
    }

    /// Índice do clip de áudio VÁLIDO alvo do editor de warp.
    pub(crate) fn tw_warp_clip(&self) -> Option<usize> {
        self.timewarp_target
            .filter(|i| *i < self.clips.len())
            .filter(|i| !self.clips[*i].is_midi)
    }

    pub(super) fn tw_warp_clip_data(&self) -> Option<&clips::ClipDemo> {
        self.tw_warp_clip().and_then(|i| self.clips.get(i))
    }

    /// Segundos que o clip ocupa no projeto (span), pelo mapa de tempo.
    pub(super) fn tw_clip_span_seconds(&self, clip: &clips::ClipDemo) -> f64 {
        let a = self.tempo_map.beats_to_seconds(clip.beat) as f64;
        let b = self.tempo_map.beats_to_seconds(clip.beat + clip.len) as f64;
        (b - a).max(1e-6)
    }

    /// Região natural da fonte em segundos. Cai no span quando o arquivo
    /// ainda não foi decodificado (region_seconds == 0).
    pub(super) fn tw_clip_region_seconds(&self, clip: &clips::ClipDemo) -> f64 {
        if clip.region_seconds > 1e-6 {
            clip.region_seconds
        } else {
            self.tw_clip_span_seconds(clip)
        }
    }

    /// `source_seconds` que mantém a proporção atual no offset pedido
    /// (interpola entre as âncoras: começo, marcadores, fim). Assim criar um
    /// marcador NÃO muda o som até o usuário arrastá-lo.
    pub(super) fn tw_warp_source_for_offset(&self, clip: &clips::ClipDemo, offset: f32) -> f32 {
        let region = self.tw_clip_region_seconds(clip);
        let mut anchors: Vec<(f32, f64)> = Vec::with_capacity(clip.warp_markers.len() + 2);
        anchors.push((0.0, 0.0));
        for mk in &clip.warp_markers {
            anchors.push((mk.project_offset_beats, mk.source_seconds as f64));
        }
        anchors.push((clip.len.max(1e-3), region));
        anchors.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let offset = offset.clamp(0.0, clip.len.max(1e-3));
        let mut src = 0.0f64;
        for pair in anchors.windows(2) {
            let (b0, s0) = pair[0];
            let (b1, s1) = pair[1];
            if offset <= b1 || (b1 - b0).abs() < 1e-6 {
                let f = if b1 > b0 {
                    ((offset - b0) / (b1 - b0)).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                src = s0 + (s1 - s0) * f as f64;
                break;
            }
        }
        (src.clamp(0.0, region)) as f32
    }

    /// Adiciona um marcador de AudioWarp no offset (beats locais) do clip alvo
    /// e escreve de volta no documento.
    pub(super) fn tw_warp_add_marker_at(&mut self, clip_beat: f32, offset: f32) {
        let Some(idx) = self.tw_warp_clip() else {
            return;
        };
        let clip = &self.clips[idx];
        let offset = offset.clamp(1e-3, (clip.len - 1e-3).max(1e-3));
        let source = self.tw_warp_source_for_offset(clip, offset);
        let _ = clip_beat;
        let mut markers = self.clips[idx].warp_markers.clone();
        markers.push(WarpMarker::new(source, offset));
        self.tw_warp_set_markers(idx, markers);
    }

    /// Move um marcador de warp para o novo offset (beats locais) e escreve.
    pub(super) fn tw_warp_move_marker(&mut self, marker_idx: usize, offset: f32) {
        let Some(idx) = self.tw_warp_clip() else {
            return;
        };
        let mut markers = self.clips[idx].warp_markers.clone();
        if let Some(mk) = markers.get_mut(marker_idx) {
            mk.project_offset_beats = offset.max(0.0);
        }
        self.tw_warp_set_markers(idx, markers);
    }

    /// Preview efêmero do marcador durante o gesto. Atualiza o cache visual
    /// do clip para que timeline e Timewarp mostrem o mesmo stretch a cada frame.
    pub(super) fn tw_warp_preview_marker(&mut self) {
        let Some(marker_idx) = self.timewarp_inter.warp_drag else {
            return;
        };
        let offset = self.timewarp_inter.warp_drag_live;
        let Some(idx) = self.tw_warp_clip() else {
            return;
        };
        self.timewarp_preview_marker = Some((idx, marker_idx, offset));
        self.request_redraw();
    }

    /// Grava os marcadores de warp no clip (engine) e persiste no documento.
    pub(super) fn tw_warp_set_markers(&mut self, clip_idx: usize, mut markers: Vec<WarpMarker>) {
        markers = daw_core::clip::normalize_warp_markers(markers);
        if let Some(clip) = self.clips.get_mut(clip_idx) {
            clip.warp_markers = markers;
        }
        self.emit_clip_write_back();
        self.request_redraw();
    }
}
