//! MIDI input handling for recording - Stub implementation.
//!
//! TODO: Replace with proper midir implementation once ALSA conflict with cpal is resolved.
//! See https://github.com/Boddlnagg/midir/issues/138

use std::sync::{Arc, Mutex, mpsc};
use crate::midi_recording::{MidiRecordEvent, MidiRecordingState};

/// Conexão MIDI ativa com callback para enviar eventos para o canal.
pub struct MidiInputManager {
    event_tx: mpsc::Sender<MidiRecordEvent>,
    connected: bool,
}

impl MidiInputManager {
    /// Cria um novo gerenciador de entrada MIDI.
    pub fn new(event_tx: mpsc::Sender<MidiRecordEvent>) -> Result<Self, String> {
        Ok(Self {
            event_tx,
            connected: false,
        })
    }

    /// Lista dispositivos MIDI de entrada disponíveis.
    pub fn list_ports(&self) -> Vec<String> {
        // Stub: retorna lista vazia
        vec!["(Nenhum dispositivo MIDI disponível - stub)".to_string()]
    }

    /// Conecta a uma porta MIDI pelo índice.
    pub fn connect_port(&mut self, _port_index: usize) -> Result<(), String> {
        // Stub: sempre falha
        Err("MIDI input não implementado (conflito ALSA/cpal). Use midir quando resolvido.".to_string())
    }

    /// Desconecta a porta atual.
    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    /// Verifica se há uma conexão ativa.
    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

/// Estado compartilhado do MIDI input para uso na UI.
#[derive(Clone)]
pub struct SharedMidiInput {
    pub state: Arc<Mutex<MidiRecordingState>>,
    pub event_tx: mpsc::Sender<MidiRecordEvent>,
    pub event_rx: Arc<Mutex<mpsc::Receiver<MidiRecordEvent>>>,
}

impl SharedMidiInput {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            state: Arc::new(Mutex::new(MidiRecordingState::default())),
            event_tx: tx,
            event_rx: Arc::new(Mutex::new(rx)),
        }
    }
}