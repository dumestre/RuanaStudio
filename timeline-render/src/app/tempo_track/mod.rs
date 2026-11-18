//! Tempo Track: faixa de tempo dedicada no topo da timeline (estilo Cubase).
//!
//! Mostra a curva de BPM ao longo da timeline e permite editar pontos de tempo
//! diretamente na arrange window, sem abrir o editor de timewarp separado.

pub mod data;
pub mod interaction;
pub mod render;

pub use data::{TempoTrackMetrics, TempoPoint, TEMPO_TRACK_HEIGHT, BPM_MIN, BPM_MAX};
pub use interaction::TempoTrackInteraction;
pub use render::build_tempo_track_instances;