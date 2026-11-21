use super::*;

impl App {
    pub fn toggle_clip_focus(&mut self, width: f32) {
        if let Some((sx, ppb, sy, zoom_y)) = self.focus_saved.take() {
            self.scroll_x = sx;
            self.px_per_beat = ppb;
            self.scroll_y = sy;
            self.zoom_y = zoom_y;
            return;
        }

        // Prioriza o primeiro clip selecionado; senÃ£o usa o hover.
        let idx = self
            .clip_interaction
            .selected
            .iter()
            .position(|&s| s)
            .or(self.clip_interaction.hover_idx);

        let Some(idx) = idx else { return };
        let Some(clip) = self.clips.get(idx) else {
            return;
        };

        self.focus_saved = Some((self.scroll_x, self.px_per_beat, self.scroll_y, self.zoom_y));

        // Centraliza horizontal e verticalmente no clip selecionado.
        let clip_x = clip.beat * self.px_per_beat;
        self.scroll_x = (clip_x - width * 0.5).max(0.0);

        let view_h = (self.size.1 as f32 - self.content_top() - self.time_ruler_h).max(0.0);
        let clip_y = self.track_top_at(clip.track_index);
        self.scroll_y = (clip_y - view_h * 0.5).max(0.0);
    }

    /// Copia os clips selecionados para o clipboard, preservando posiÃ§Ãµes relativas.
    pub fn copy_selected(&mut self) {
        let selected_clips: Vec<clips::ClipDemo> = self
            .clips
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                i < &self.clip_interaction.selected.len() && self.clip_interaction.selected[*i]
            })
            .map(|(_, c)| c.clone())
            .collect();
        if !selected_clips.is_empty() {
            self.clipboard = selected_clips;
        }
    }

    /// Seleciona todos os clips da timeline.
    pub fn select_all(&mut self) {
        self.clip_interaction.sync_selection(self.clips.len());
        self.clip_interaction.select_all();
    }

    /// Registra um snapshot do estado atual (clips + fades) na pilha de undo.
    /// Limpa a pilha de redo (novo ramo de ediÃ§Ã£o). Deve ser chamado ANTES de
    /// qualquer mutaÃ§Ã£o estrutural (import, paste, slice, erase, etc.).
    pub fn record_undo(&mut self) {
        use crate::app::TimelineSnapshot;
        self.undo_stack.push(TimelineSnapshot {
            clips: self.clips.clone(),
            fades: self.fades.clone(),
        });
        self.redo_stack.clear();
    }

    /// Desfaz a Ãºltima ediÃ§Ã£o estrutural (clips + fades).
    pub fn undo(&mut self) {
        use crate::app::TimelineSnapshot;
        if let Some(snap) = self.undo_stack.pop() {
            // O estado atual vai para o redo (permite refazer).
            self.redo_stack.push(TimelineSnapshot {
                clips: self.clips.clone(),
                fades: self.fades.clone(),
            });
            self.clips = snap.clips;
            self.fades = snap.fades;
            self.clip_interaction.sync_selection(self.clips.len());
            self.rebuild_wave_data();
        }
    }

    /// Refaz a ediÃ§Ã£o que foi desfeita.
    pub fn redo(&mut self) {
        use crate::app::TimelineSnapshot;
        if let Some(snap) = self.redo_stack.pop() {
            self.undo_stack.push(TimelineSnapshot {
                clips: self.clips.clone(),
                fades: self.fades.clone(),
            });
            self.clips = snap.clips;
            self.fades = snap.fades;
            self.clip_interaction.sync_selection(self.clips.len());
            self.rebuild_wave_data();
        }
    }

    /// Cola os clips do clipboard na posiÃ§Ã£o alvo (beat, track), com snap ao grid dinâmico.
    /// MantÃ©m a estrutura relativa dos clips copiados.
    pub fn paste_at_beat(&mut self, target_beat: f32, target_track: usize) {
        if self.clipboard.is_empty() {
            return;
        }
        self.record_undo();
        // Encontra o beat mÃ­nimo dos clips copiados para manter offsets relativos
        let min_beat = self
            .clipboard
            .iter()
            .map(|c| c.beat)
            .fold(f32::INFINITY, f32::min);
        let snap_beat = if self.snap_enabled {
            let step = self.grid_beat_step();
            (target_beat / step).round() * step
        } else {
            target_beat
        }
        .max(0.0);
        let beat_offset = snap_beat - min_beat;
        // Track offset: usa a track do primeiro clip copiado como referÃªncia
        let min_track = self
            .clipboard
            .iter()
            .map(|c| c.track_index)
            .min()
            .unwrap_or(0);
        let track_offset = target_track as i32 - min_track as i32;
        let max_track = self.clips.iter().map(|c| c.track_index).max().unwrap_or(0);
        for clip in &self.clipboard {
            let mut new_clip = clip.clone();
            new_clip.id = 0;
            new_clip.beat = (clip.beat + beat_offset).max(0.0);
            new_clip.track_index = (clip.track_index as i32 + track_offset)
                .clamp(0, (max_track + 1).max(1) as i32 - 1)
                as usize;
            self.clips.push(new_clip);
        }
        // Seleciona os clips colados
        self.clip_interaction.sync_selection(self.clips.len());
        for s in self.clip_interaction.selected.iter_mut() {
            *s = false;
        }
        let pasted_count = self.clipboard.len();
        for i in (self.clips.len() - pasted_count)..self.clips.len() {
            if i < self.clip_interaction.selected.len() {
                self.clip_interaction.selected[i] = true;
            }
        }
        self.rebuild_wave_data();
        self.emit_clip_write_back();
    }

    /// Duplica os clips selecionados APÃ“S o Ãºltimo clip selecionado (com snap ao grid).
    pub fn duplicate_selected(&mut self) {
        let selected_indices: Vec<usize> = self
            .clip_interaction
            .selected
            .iter()
            .enumerate()
            .filter(|&(_, &s)| s)
            .map(|(i, _)| i)
            .collect();
        if selected_indices.is_empty() {
            return;
        }
        self.record_undo();
        // Encontra o clip que termina mais tarde (maior beat + len)
        let max_end = selected_indices
            .iter()
            .map(|&i| self.clips[i].beat + self.clips[i].len)
            .fold(0.0_f32, f32::max);
        let snap_beat = if self.snap_enabled {
            let step = self.grid_beat_step();
            (max_end / step).ceil() * step
        } else {
            max_end
        }
        .max(0.0);
        let clips_to_dup: Vec<clips::ClipDemo> = selected_indices
            .iter()
            .map(|&i| self.clips[i].clone())
            .collect();
        let min_beat = clips_to_dup
            .iter()
            .map(|c| c.beat)
            .fold(f32::INFINITY, f32::min);
        let beat_offset = snap_beat - min_beat;
        for clip in &clips_to_dup {
            let mut new_clip = clip.clone();
            new_clip.id = 0;
            new_clip.beat = (clip.beat + beat_offset).max(0.0);
            self.clips.push(new_clip);
        }
        // Seleciona os duplicados
        self.clip_interaction.sync_selection(self.clips.len());
        for s in self.clip_interaction.selected.iter_mut() {
            *s = false;
        }
        let dup_count = clips_to_dup.len();
        for i in (self.clips.len() - dup_count)..self.clips.len() {
            if i < self.clip_interaction.selected.len() {
                self.clip_interaction.selected[i] = true;
            }
        }
        self.rebuild_wave_data();
        self.emit_clip_write_back();
    }

    /// Deleta os clips selecionados.
    pub fn delete_selected(&mut self) {
        let selected_indices: Vec<usize> = self
            .clip_interaction
            .selected
            .iter()
            .enumerate()
            .filter(|&(_, &s)| s)
            .map(|(i, _)| i)
            .collect();
        if selected_indices.is_empty() {
            return;
        }
        self.record_undo();
        // Remove em ordem reversa para manter Ã­ndices vÃ¡lidos
        for &i in selected_indices.iter().rev() {
            self.clips.remove(i);
        }
        self.clip_interaction.sync_selection(self.clips.len());
        self.rebuild_wave_data();
        self.emit_clip_write_back();
    }

    /// Dispara o write-back estruturado de clips para o host (hosted).
    pub(super) fn emit_clip_write_back(&mut self) {
        if let Some(cb) = &mut self.clip_write_back {
            cb(&self.clips);
        }
    }

    /// Avisa o host que o usuário quer EDITAR um clip (duplo clique). Clip
    /// sem id real (recém-desenhado, ainda `id == 0`) é ignorado: o host
    /// não conseguiria resolve-lo no `Project`.
    pub(crate) fn emit_clip_open_request(&mut self, id: u32) {
        if id == 0 {
            return;
        }
        if let Some(cb) = &mut self.clip_open_request {
            cb(id);
        }
    }

    /// Detecta duplo clique: dois presses no MESMO clip, dentro da janela de
    /// tempo e perto o suficiente em pixels. Zera o estado ao detectar, para
    /// que um terceiro clique não abra o editor de novo.
    pub(crate) fn detect_double_click(&mut self, pos: (f32, f32), clip: usize) -> bool {
        let now = std::time::Instant::now();
        double_click_hit(&mut self.last_press, now, pos, clip)
    }

    /// Converte posiÃ§Ã£o do cursor para beat e track com snap ao grid
    /// (respeitando `snap_enabled`: OFF mantém beat fracionário).
    /// O snap segue o grid visual dinâmico: quanto mais zoom, mais fino o grid.
    pub fn cursor_to_snapped_beat_track(&self, pos: (f32, f32)) -> (f32, usize) {
        let beat = (pos.0 + self.scroll_x) / self.px_per_beat.max(0.001);
        let snapped_beat = if self.snap_enabled {
            let step = self.grid_beat_step();
            (beat / step).round() * step
        } else {
            beat
        }
        .max(0.0);
        let content_y = (pos.1 - self.content_top() + self.scroll_y).max(0.0);
        let (track, _) = self.track_at_content_y(content_y);
        (snapped_beat, track)
    }

    /// Desenha novo clip (ferramenta Draw/Pencil).
    #[allow(dead_code)]
    pub fn tool_draw(&mut self, pos: (f32, f32)) {
        let (beat, track) = self.cursor_to_snapped_beat_track(pos);
        let len = 4.0; // comprimento padrÃ£o: 1 compasso
        self.clips.push(clips::ClipDemo {
            id: 0,
            track_index: track,
            beat,
            len,
            is_midi: false,
            color: 0,
            name: None,
            seam_beat: None,
            pre_glue_len: None,
            pre_glue_color: None,
            wave: Vec::new(),
            notes: Vec::new(),
            file_path: None,
            source_start_beats: 0.0,
            source_length_beats: 0.0,
            channels: 1,
            region_seconds: 0.0,
            warp_markers: Vec::new(),
        });
        self.clip_interaction.sync_selection(self.clips.len());
        self.rebuild_wave_data();
        self.emit_clip_write_back();
    }

    /// Estica clip (ferramenta Stretch) - arrasta borda direita.
    pub fn tool_stretch_start(&mut self, pos: (f32, f32)) -> bool {
        let cm = self.clip_metrics();
        if let Some(idx) = self.clip_interaction.hit_test(&self.clips, pos, &cm) {
            // Verifica se clicou na borda direita do clip (Ãºltimos 12px)
            let (cx, _, cw, _) = clips::clip_rect(&self.clips[idx], &cm);
            if pos.0 >= cx + cw - 12.0 {
                self.clip_interaction.begin_stretch(&self.clips, pos, &cm);
                self.clip_interaction.select_only(idx);
                self.edge_scroll_x = 0.0;
                self.edge_scroll_y = 0.0;
                self.edge_scroll_dir = 0;
                return true;
            }
        }
        false
    }

    /// Corta clip (ferramenta Slice â€” tecla 4).
    /// Divide o clip no ponto do clique. O clip da esquerda (original) Ã© selecionado,
    /// entÃ£o cortes consecutivos deixam o clip do meio selecionado automaticamente.
    pub fn tool_slice(&mut self, pos: (f32, f32)) {
        let cm = self.clip_metrics();
        if let Some(idx) = self.clip_interaction.hit_test(&self.clips, pos, &cm) {
            let clip = self.clips[idx].clone();
            let click_beat = (pos.0 + self.scroll_x) / self.px_per_beat.max(0.001);
            let click_beat_snapped = if self.snap_enabled {
                let step = self.grid_beat_step();
                (click_beat / step).round() * step
            } else {
                click_beat
            }
            .max(clip.beat)
            .min(clip.beat + clip.len);

            // SÃ³ divide se o clique nÃ£o estiver na borda
            if click_beat_snapped > clip.beat && click_beat_snapped < clip.beat + clip.len {
                self.record_undo();
                let left_len = click_beat_snapped - clip.beat;
                let right_len = clip.beat + clip.len - click_beat_snapped;

                // Atualiza clip original para a parte esquerda
                self.clips[idx].len = left_len;

                // Cria novo clip para a parte direita
                let mut right_clip = clip;
                right_clip.id = 0;
                right_clip.beat = click_beat_snapped;
                right_clip.len = right_len;

                // Corte VERDADEIRO da waveform: divide as amostras do clip
                // pai, proporcional ao ponto de corte. As duas peÃ§as exibem
                // exatamente o conteÃºdo original, apenas cortado â€” nunca uma
                // onda regenerada.
                let parent_len = left_len + right_len;
                if left_len >= 0.25 && right_len >= 0.25 && self.clips[idx].wave.len() > 1 {
                    let n = self.clips[idx].wave.len();
                    let frac = (left_len / parent_len).clamp(0.0, 1.0);
                    let split_at = ((n as f32 * frac).round() as usize).clamp(1, n - 1);
                    let right_wave = self.clips[idx].wave[split_at..].to_vec();
                    self.clips[idx].wave.truncate(split_at);
                    right_clip.wave = right_wave;
                } else {
                    right_clip.wave = Vec::new();
                }
                self.clips.push(right_clip);

                self.clip_interaction.sync_selection(self.clips.len());
                // Seleciona o clip esquerdo (parte que foi cortada) â€”
                // cortes consecutivos mantÃªm o clip do meio selecionado.
                self.clip_interaction.select_only(idx);
                // A segmentaÃ§Ã£o do buffer muda (um clip virou dois): regenera
                // para o clip direito tambÃ©m ter waveform.
                self.rebuild_wave_data();
                self.emit_clip_write_back();
            }
        }
    }

    /// Apaga clip (ferramenta Erase) - clica para deletar.
    pub fn tool_erase(&mut self, pos: (f32, f32)) {
        let cm = self.clip_metrics();
        if let Some(idx) = self.clip_interaction.hit_test(&self.clips, pos, &cm) {
            self.record_undo();
            self.clips.remove(idx);
            self.clip_interaction.sync_selection(self.clips.len());
            self.rebuild_wave_data();
            self.emit_clip_write_back();
        }
    }

    /// Cola clips (ferramenta Glue â€” tecla 6).
    /// Clicar num clip: estica a borda direita dele (ou do Ãºltimo da cadeia)
    /// atÃ© o inÃ­cio do prÃ³ximo clip na mesma track, preenchendo o gap.
    /// Ambos permanecem como clips separados, mas adjacentes (sem gap),
    /// com a mesma cor. O seam marca o ponto de encontro para des-colagem.
    /// Clicar de novo no mesmo clip continua a cadeia grudando no prÃ³ximo.
    pub fn tool_glue(&mut self, pos: (f32, f32)) {
        let cm = self.clip_metrics();
        if let Some(idx) = self.clip_interaction.hit_test(&self.clips, pos, &cm) {
            let track = self.clips[idx].track_index;

            // Encontra o final da cadeia de glue (Ãºltimo clip com seam)
            let chain = clips::ClipInteraction::find_group(&self.clips, idx);
            let chain_end_idx = *chain.last().unwrap();

            let end_clip = self.clips[chain_end_idx].clone();
            let current_end = end_clip.beat + end_clip.len;

            let next_idx = self
                .clips
                .iter()
                .enumerate()
                .filter(|(i, c)| {
                    c.track_index == track && !chain.contains(i) && c.beat >= current_end - 0.01
                })
                .min_by_key(|(_, c)| (c.beat * 1000.0) as i32)
                .map(|(i, _)| i);

            if let Some(next_idx) = next_idx {
                self.record_undo();
                let next_beat = self.clips[next_idx].beat;
                if self.clips[chain_end_idx].pre_glue_len.is_none() {
                    self.clips[chain_end_idx].pre_glue_len = Some(end_clip.len);
                }
                self.clips[chain_end_idx].seam_beat = Some(next_beat);
                if self.clips[next_idx].pre_glue_color.is_none() {
                    self.clips[next_idx].pre_glue_color = Some(self.clips[next_idx].color);
                }
                self.clips[next_idx].color = end_clip.color;
                self.clips[chain_end_idx].len = (next_beat - end_clip.beat).max(0.25);
                self.clip_interaction.select_chain(&self.clips, &chain);
                self.rebuild_wave_data();
                self.emit_clip_write_back();
            }
        }
    }

    /// Inicia a reproduÃ§Ã£o a partir de um beat especificado.
    pub(super) fn start_playback_at(&mut self, beat: f32) {
        // Converte beat para segundos.
        self.transport_seconds = beat * 60.0 / self.bpm;
        // Sincroniza o motor de Ã¡udio.
        #[cfg(feature = "audio")]
        self.sync_audio_engine();
        // Configura o transporte e inicia a reproduÃ§Ã£o.
        if let Ok(mut t) = self.transport.lock() {
            t.bpm = self.bpm as u32;
            t.loop_enabled = self.loop_enabled;
            if self.loop_enabled {
                t.loop_start_ticks = (self.loop_start_beat * t.ppq as f32) as u64;
                t.loop_end_ticks = (self.loop_end_beat * t.ppq as f32) as u64;
            }
            let beats = self.transport_seconds * self.bpm / 60.0;
            t.position_ticks = (beats * t.ppq as f32) as u64;
            t.playing = true;
        }
        self.play_start_seconds = self.transport_seconds;
        self.playing = true;
        self.last_play_head_update = Instant::now();
        self.request_redraw();
    }
}
