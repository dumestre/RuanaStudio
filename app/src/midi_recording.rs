//! MIDI recording types and state.

use crate::daw_midi_reexport::Note;
use serde::{Deserialize, Serialize};

/// Evento MIDI recebido durante gravação.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MidiRecordEvent {
    NoteOn {
        channel: u8,
        pitch: i32,
        velocity: i32,
    },
    NoteOff {
        channel: u8,
        pitch: i32,
    },
    ControlChange {
        channel: u8,
        controller: i32,
        value: i32,
    },
    PitchBend {
        channel: u8,
        value: i32,
    },
    ProgramChange {
        channel: u8,
        program: i32,
    },
    ChannelPressure {
        channel: u8,
        pressure: i32,
    },
    PolyPressure {
        channel: u8,
        pitch: i32,
        pressure: i32,
    },
}

/// Nota gravada com timestamp em beats.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RecordedNote {
    pub pitch: i32,
    pub start_beat: f32,
    pub velocity: i32,
    pub channel: u8,
}

/// Estado da gravação MIDI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MidiRecordingState {
    /// Se a gravação está armada (pronta para gravar ao dar play).
    pub armed: bool,
    /// Se está gravando ativamente no momento.
    pub recording: bool,
    /// Track ID alvo para gravação.
    pub target_track_id: Option<u64>,
    /// Clip ID sendo gravado (criado ao iniciar gravação).
    pub recording_clip_id: Option<u64>,
    /// Dispositivo MIDI selecionado (índice na lista).
    pub selected_device: Option<usize>,
    /// Canal MIDI para gravar (0 = todos, 1-16 = específico).
    pub record_channel: u8,
    /// Quantização durante gravação (denominador: 4=1/4, 8=1/8, 16=1/16, 0=off).
    pub quantize_denom: i32,
    /// Modo de gravação: overwrite ou overdub.
    pub overdub: bool,
    /// Count-in em compassos antes de começar a gravar.
    pub count_in_bars: u32,
    /// Notas gravadas na sessão atual (buffer).
    pub recorded_notes: Vec<RecordedNote>,
    /// Notas pendentes (NoteOn sem NoteOff correspondente).
    pub pending_notes: Vec<PendingNote>,
    /// Beat position quando a gravação iniciou.
    pub recording_start_beat: f32,
}

/// Nota pendente aguardando NoteOff.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PendingNote {
    pub pitch: i32,
    pub start_beat: f32,
    pub velocity: i32,
    pub channel: u8,
}

impl MidiRecordingState {
    /// Inicia uma nova sessão de gravação.
    pub fn start_recording(&mut self, track_id: u64, clip_id: u64, current_beat: f32) {
        self.recording = true;
        self.target_track_id = Some(track_id);
        self.recording_clip_id = Some(clip_id);
        self.recorded_notes.clear();
        self.pending_notes.clear();
        self.recording_start_beat = current_beat;
    }

    /// Para a gravação.
    pub fn stop_recording(&mut self) {
        self.recording = false;
        // Finaliza notas pendentes no final da gravação
        for pending in self.pending_notes.drain(..) {
            self.recorded_notes.push(RecordedNote {
                pitch: pending.pitch,
                start_beat: pending.start_beat,
                velocity: pending.velocity,
                channel: pending.channel,
            });
        }
    }

    /// Processa um evento MIDI durante gravação.
    pub fn process_event(&mut self, event: MidiRecordEvent, current_beat: f32) {
        if !self.recording {
            return;
        }

        // Filtro de canal
        let event_channel = match &event {
            MidiRecordEvent::NoteOn { channel, .. } => *channel,
            MidiRecordEvent::NoteOff { channel, .. } => *channel,
            MidiRecordEvent::ControlChange { channel, .. } => *channel,
            MidiRecordEvent::PitchBend { channel, .. } => *channel,
            MidiRecordEvent::ProgramChange { channel, .. } => *channel,
            MidiRecordEvent::ChannelPressure { channel, .. } => *channel,
            MidiRecordEvent::PolyPressure { channel, .. } => *channel,
        };

        if self.record_channel != 0 && event_channel != self.record_channel {
            return;
        }

        match event {
            MidiRecordEvent::NoteOn { channel, pitch, velocity } => {
                // Adiciona à lista de pendentes
                self.pending_notes.push(PendingNote {
                    pitch,
                    start_beat: current_beat - self.recording_start_beat,
                    velocity,
                    channel,
                });
            }
            MidiRecordEvent::NoteOff { channel, pitch } => {
                // Procura nota pendente correspondente
                if let Some(idx) = self.pending_notes.iter().position(|n| n.pitch == pitch && n.channel == channel) {
                    let pending = self.pending_notes.remove(idx);
                    self.recorded_notes.push(RecordedNote {
                        pitch: pending.pitch,
                        start_beat: pending.start_beat,
                        velocity: pending.velocity,
                        channel: pending.channel,
                    });
                }
            }
            _ => {
                // Outros eventos (CC, pitch bend, etc.) podem ser gravados futuramente
            }
        }
    }

    /// Aplica quantização nas notas gravadas.
    pub fn apply_quantize(&mut self) {
        if self.quantize_denom <= 0 {
            return;
        }
        let step = 4.0 / self.quantize_denom as f32;
        for note in &mut self.recorded_notes {
            note.start_beat = (note.start_beat / step).round() * step;
            if note.start_beat < 0.0 {
                note.start_beat = 0.0;
            }
        }
    }

    /// Converte notas gravadas para daw_midi::Note.
    pub fn to_daw_midi_notes(&self) -> Vec<crate::daw_midi_reexport::Note> {
        self.recorded_notes.iter().enumerate().map(|(i, n)| {
            crate::daw_midi_reexport::Note {
                id: i as i32 + 1,
                pitch: n.pitch,
                start: n.start_beat,
                length: 0.25, // Duração padrão, será ajustada pelo NoteOff real
                velocity: n.velocity,
            }
        }).collect()
    }
}