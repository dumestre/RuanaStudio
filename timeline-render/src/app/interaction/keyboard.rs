use super::*;

impl App {
    pub fn grid_beat_step(&self) -> f32 {
        let px_per_bar = self.px_per_beat * 4.0;
        if px_per_bar >= 960.0 {
            1.0 / 16.0
        } else if px_per_bar >= 480.0 {
            1.0 / 8.0
        } else if px_per_bar >= 240.0 {
            1.0 / 4.0
        } else if px_per_bar >= 120.0 {
            1.0 / 2.0
        } else {
            1.0
        }
    }

    /// Atalhos de teclado do motor (mesma tecla do renderer standalone):
    /// `1..6` ferramentas, `P` piano roll, `F`/`Ctrl+F` foco/follow, `Ctrl+C/A/
    /// Z/Y/V/D`, `Delete/Backspace`, e o rastreio de modificadores. EspaÃ§o e
    /// Esc apenas sinalizam (`TogglePlay`/`ExitWindow`) â€” o motor NÃƒO toca no
    /// transporte nem fecha nada.
    pub fn handle_key(&mut self, event: &winit::event::KeyEvent) -> KeyAction {
        use winit::event::ElementState;
        use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

        let pressed = event.state == ElementState::Pressed;
        let vk = &event.logical_key;

        if pressed {
            if *vk == Key::Named(NamedKey::Space)
                || matches!(vk, Key::Character(s) if s.as_ref() == " ")
            {
                return KeyAction::TogglePlay;
            }
            if *vk == Key::Named(NamedKey::Escape) {
                return KeyAction::ExitWindow;
            }

            // P: alterna o piano roll.
            if matches!(vk, Key::Character(s) if s.eq_ignore_ascii_case("p")) {
                self.toggle_piano_roll();
                self.toolbar_hover = None;
                self.request_redraw();
            }
            // T: alterna o timewarp (exclusivo com o piano roll).
            if matches!(vk, Key::Character(s) if s.eq_ignore_ascii_case("t")) {
                self.toggle_timewarp();
                self.toolbar_hover = None;
                self.request_redraw();
            }
            // F: foca o clip sob o cursor; Ctrl+F: toggle follow playhead.
            if matches!(vk, Key::Character(s) if s.eq_ignore_ascii_case("f")) {
                if self.ctrl_pressed {
                    self.follow_playhead = !self.follow_playhead;
                } else {
                    self.toggle_clip_focus(self.size.0 as f32);
                }
                self.request_redraw();
            }
            // Ferramentas (1-6).
            if !self.ctrl_pressed && !self.alt_pressed {
                let tool = match vk {
                    Key::Character(s) if s == "1" => Some(Tool::Select),
                    Key::Character(s) if s == "2" => Some(Tool::Draw),
                    Key::Character(s) if s == "3" => Some(Tool::Stretch),
                    Key::Character(s) if s == "4" => Some(Tool::Slice),
                    Key::Character(s) if s == "5" => Some(Tool::Erase),
                    Key::Character(s) if s == "6" => Some(Tool::Glue),
                    _ => None,
                };
                if let Some(tool) = tool {
                    set_tool(tool);
                    self.request_redraw();
                }
            }
            // Clipboard/editing (Ctrl+C, Ctrl+V, Ctrl+D, Ctrl+A, Ctrl+Z,
            // Ctrl+Y, Delete, Backspace).
            if self.ctrl_pressed {
                match vk {
                    Key::Character(s) if s.eq_ignore_ascii_case("c") => {
                        self.copy_selected();
                        self.request_redraw();
                    }
                    Key::Character(s) if s.eq_ignore_ascii_case("a") => {
                        self.select_all();
                        self.request_redraw();
                    }
                    Key::Character(s) if s.eq_ignore_ascii_case("z") => {
                        if self.shift_pressed {
                            self.redo();
                        } else {
                            self.undo();
                        }
                        self.request_redraw();
                    }
                    Key::Character(s) if s.eq_ignore_ascii_case("y") => {
                        self.redo();
                        self.request_redraw();
                    }
                    Key::Character(s) if s.eq_ignore_ascii_case("v") => {
                        // Cola na posiÃ§Ã£o do playhead (com snap) na track do
                        // primeiro clip selecionado ou 0.
                        let bpm = self.bpm;
                        let pps = self.px_per_beat * bpm / 60.0;
                        let target_beat = if pps > 0.0 {
                            self.transport_seconds * bpm / 60.0
                        } else {
                            0.0
                        };
                        let target_track = self
                            .clip_interaction
                            .selected
                            .iter()
                            .enumerate()
                            .find(|&(_, &s)| s)
                            .and_then(|(i, _)| self.clips.get(i))
                            .map(|c| c.track_index)
                            .unwrap_or(0);
                        self.paste_at_beat(target_beat, target_track);
                        self.request_redraw();
                    }
                    Key::Character(s) if s.eq_ignore_ascii_case("d") => {
                        self.duplicate_selected();
                        self.request_redraw();
                    }
                    _ => {}
                }
            }
            if matches!(
                vk,
                Key::Named(NamedKey::Delete) | Key::Named(NamedKey::Backspace)
            ) {
                if self.show_piano_roll {
                    let ids: Vec<i32> = self
                        .piano_roll_inter
                        .selected
                        .iter()
                        .enumerate()
                        .filter(|&(_, selected)| *selected)
                        .filter_map(|(index, _)| {
                            self.piano_roll_notes.get(index).map(|note| note.id)
                        })
                        .collect();
                    if !ids.is_empty() {
                        self.note_editor.delete_many(&ids);
                        self.sync_piano_roll_from_core();
                        self.emit_piano_roll_write_back();
                    }
                } else {
                    self.delete_selected();
                }
                self.request_redraw();
            }
        }

        // Track ctrl/alt/shift p/ zoom e shortcuts â€” sÃ³ quando a tecla Ã‰ o
        // modificador.
        let is_ctrl_key = matches!(vk, Key::Named(NamedKey::Control))
            || matches!(event.physical_key,
                PhysicalKey::Code(code)
                    if matches!(code, KeyCode::ControlLeft | KeyCode::ControlRight));
        let is_alt_key = matches!(vk, Key::Named(NamedKey::Alt))
            || matches!(event.physical_key,
                PhysicalKey::Code(code)
                    if matches!(code, KeyCode::AltLeft | KeyCode::AltRight));
        let is_shift_key = matches!(vk, Key::Named(NamedKey::Shift))
            || matches!(event.physical_key,
                PhysicalKey::Code(code)
                    if matches!(code, KeyCode::ShiftLeft | KeyCode::ShiftRight));
        if is_ctrl_key {
            self.ctrl_pressed = pressed;
        }
        if is_alt_key {
            self.alt_pressed = pressed;
        }
        if is_shift_key {
            self.shift_pressed = pressed;
        }

        // Qualquer tecla pressionada marca o frame como sujo (a cena da
        // timeline re-compÃµe â€” no hosted o prÃ³ximo `tick` re-renderiza).
        if pressed {
            self.request_redraw();
        }

        KeyAction::None
    }
    pub fn handle_scroll(&mut self, dx: f32, dy: f32, with_ctrl: bool, with_alt: bool) {
        let (mx, my) = self.last_cursor;

        // Determina qual área está sob o cursor: timeline, piano roll ou
        // timewarp (cada uma com zoom/scroll INDEPENDENTES).
        let layout = self.layout(self.size.0 as f32, self.size.1 as f32);
        let (px_per_beat, scroll_x, scroll_y, in_pr_area, in_tw_area) = if self.show_timewarp {
            let tw = layout.timewarp.as_ref();
            if let Some(tw) = tw {
                if my >= tw.y && my <= tw.y + tw.height {
                    (
                        self.timewarp_px_per_beat,
                        self.timewarp_scroll_x,
                        0.0,
                        false,
                        true,
                    )
                } else {
                    (self.px_per_beat, self.scroll_x, self.scroll_y, false, false)
                }
            } else {
                (self.px_per_beat, self.scroll_x, self.scroll_y, false, false)
            }
        } else if self.show_piano_roll {
            let pr = layout.piano_roll.as_ref();
            if let Some(pr) = pr {
                if my >= pr.y && my <= pr.y + pr.height {
                    (
                        self.piano_roll_px_per_beat,
                        self.piano_roll_scroll_x,
                        self.piano_roll_scroll_y,
                        true,
                        false,
                    )
                } else {
                    (self.px_per_beat, self.scroll_x, self.scroll_y, false, false)
                }
            } else {
                (self.px_per_beat, self.scroll_x, self.scroll_y, false, false)
            }
        } else {
            (self.px_per_beat, self.scroll_x, self.scroll_y, false, false)
        };

        if with_ctrl && dy.abs() >= 1.0 {
            let old_px = px_per_beat;
            let new_px = (old_px + dy * 0.05).clamp(8.0, 640.0);
            let anchor_x = if in_tw_area {
                mx - crate::app::timewarp::BPM_LABEL_W
            } else {
                mx
            };
            let beat_under = (anchor_x + scroll_x) / old_px.max(0.01);
            if in_tw_area {
                self.timewarp_px_per_beat = new_px;
                self.timewarp_scroll_x = (beat_under * new_px - anchor_x).max(0.0);
            } else if in_pr_area {
                self.piano_roll_px_per_beat = new_px;
                self.piano_roll_scroll_x = (beat_under * new_px - mx).max(0.0);
            } else {
                self.px_per_beat = new_px;
                self.scroll_x = (beat_under * new_px - mx).max(0.0);
            }
        } else if with_alt && dy.abs() >= 1.0 {
            if in_pr_area {
                let old_kh = self.piano_roll_key_height;
                let new_kh = (old_kh + dy * 0.1).clamp(2.0, 24.0);
                self.piano_roll_key_height = new_kh;
            } else if !in_tw_area {
                let old_zy = self.zoom_y;
                let new_zy = (old_zy + dy * 0.01).clamp(0.25, 6.0);
                let content_top = self.content_top();
                // Ancoragem: mantém sob o cursor a MESMA fração da track
                // (alturas individuais podem diferir da grade de 64px).
                let content_y = (my - content_top + scroll_y).max(0.0);
                let (track_under, local) = self.track_at_content_y(content_y);
                let old_h = self.track_height_at(track_under).max(0.01);
                let frac = (local / old_h).clamp(0.0, 1.0);
                self.zoom_y = new_zy;
                let new_local = frac * self.track_height_at(track_under);
                self.scroll_y =
                    (self.track_top_at(track_under) + new_local - (my - content_top)).max(0.0);
            }
        } else {
            if in_pr_area {
                self.piano_roll_scroll_y = (self.piano_roll_scroll_y - dy).max(0.0);
                self.piano_roll_scroll_x = (self.piano_roll_scroll_x - dx).max(0.0);
            } else if in_tw_area {
                // Timewarp não tem scroll vertical (faixa BPM fixa).
                self.timewarp_scroll_x = (self.timewarp_scroll_x - dx).max(0.0);
            } else {
                self.scroll_x = (self.scroll_x - dx).max(0.0);
                self.scroll_y = (self.scroll_y - dy).max(0.0);
            }
        }
        self.clamp_scroll_y(self.input_view_size().1);
        self.request_redraw();
    }
}
