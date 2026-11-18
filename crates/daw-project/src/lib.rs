//! Formato de arquivo de projeto da Ruana Studio.
//!
//! O [`daw_core::project::Project`] é a fonte de verdade em memória e **não**
//! implementa `serde` de propósito: ele carrega `History`, subscribers e
//! tipos internos que não fazem parte do documento. Este crate define o
//! **DTO de persistência** ([`ProjectFile`]), converte nos dois sentidos e
//! cuida do disco.
//!
//! O arquivo é JSON versionado. Campo novo entra com `#[serde(default)]`
//! para que projetos antigos continuem abrindo; mudança incompatível sobe o
//! `FORMAT_VERSION` e passa a ser recusada.

use std::fmt;
use std::path::Path;

use daw_core::clip::{Clip, ClipKind, TimeMode, WarpMarker};
use daw_core::mixer::{InsertSlot, MixerChannel};
use daw_core::project::Project;
use daw_core::timeline::{TempoChange, TempoMap};
use daw_core::track::{Track, TrackKind};
use daw_core::transport::{PlayMode, Transport};
use daw_midi::pattern::{BeatPatterns, DrumPattern, DrumTrackParams};
use serde::{Deserialize, Serialize};

/// Versão do formato gravado por este crate.
pub const FORMAT_VERSION: u32 = 1;

/// Extensão usada nos arquivos de projeto (sem o ponto).
pub const FILE_EXTENSION: &str = "ruana";

// =============================================================================
// Erros
// =============================================================================

/// Falha ao ler, gravar ou interpretar um projeto.
#[derive(Debug)]
pub enum ProjectError {
    /// Falha de I/O no disco.
    Io(std::io::Error),
    /// JSON inválido ou incompatível com o DTO.
    Json(serde_json::Error),
    /// Arquivo de uma versão futura/incompatível.
    UnsupportedVersion { found: u32, supported: u32 },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProjectError::Io(e) => write!(f, "falha de I/O: {e}"),
            ProjectError::Json(e) => write!(f, "projeto inválido: {e}"),
            ProjectError::UnsupportedVersion { found, supported } => write!(
                f,
                "projeto na versão {found}, este build abre até a {supported}"
            ),
        }
    }
}

impl std::error::Error for ProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ProjectError::Io(e) => Some(e),
            ProjectError::Json(e) => Some(e),
            ProjectError::UnsupportedVersion { .. } => None,
        }
    }
}

impl From<std::io::Error> for ProjectError {
    fn from(e: std::io::Error) -> Self {
        ProjectError::Io(e)
    }
}

impl From<serde_json::Error> for ProjectError {
    fn from(e: serde_json::Error) -> Self {
        ProjectError::Json(e)
    }
}

/// Resultado das operações deste crate.
pub type Result<T> = std::result::Result<T, ProjectError>;

// =============================================================================
// DTOs
// =============================================================================

/// Documento persistido (raiz do arquivo).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFile {
    /// Versão do formato (ver [`FORMAT_VERSION`]).
    #[serde(default = "default_version")]
    pub version: u32,
    /// Nome do projeto (título da janela / recentes).
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tracks: Vec<TrackDto>,
    #[serde(default)]
    pub clips: Vec<ClipDto>,
    #[serde(default)]
    pub tempo_map: TempoMapDto,
    #[serde(default)]
    pub master_mixer: MixerDto,
    #[serde(default)]
    pub beat_patterns: BeatPatternsDto,
    /// Transporte (playhead, loop, metrônomo). Opcional: um projeto recém
    /// criado não tem transporte salvo ainda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<TransportDto>,
}

fn default_version() -> u32 {
    FORMAT_VERSION
}

/// Trilha.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackDto {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: TrackKindDto,
    #[serde(default)]
    pub color: u8,
    #[serde(default)]
    pub mixer: MixerDto,
}

/// Tipo de trilha (espelho serializável de [`daw_core::track::TrackKind`]).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKindDto {
    /// Trilha de áudio.
    #[default]
    Audio,
    /// Trilha MIDI.
    Midi,
    /// Trilha de instrumento.
    Instrument,
    /// Trilha de bateria.
    Drum,
    /// Trilha de efeitos.
    Fx,
    /// Trilha barramento.
    Bus,
    /// Trilha master.
    Master,
}

impl From<TrackKind> for TrackKindDto {
    fn from(k: TrackKind) -> Self {
        match k {
            TrackKind::Audio => TrackKindDto::Audio,
            TrackKind::Midi => TrackKindDto::Midi,
            TrackKind::Instrument => TrackKindDto::Instrument,
            TrackKind::Drum => TrackKindDto::Drum,
            TrackKind::Fx => TrackKindDto::Fx,
            TrackKind::Bus => TrackKindDto::Bus,
            TrackKind::Master => TrackKindDto::Master,
        }
    }
}

impl From<TrackKindDto> for TrackKind {
    fn from(k: TrackKindDto) -> Self {
        match k {
            TrackKindDto::Audio => TrackKind::Audio,
            TrackKindDto::Midi => TrackKind::Midi,
            TrackKindDto::Instrument => TrackKind::Instrument,
            TrackKindDto::Drum => TrackKind::Drum,
            TrackKindDto::Fx => TrackKind::Fx,
            TrackKindDto::Bus => TrackKind::Bus,
            TrackKindDto::Master => TrackKind::Master,
        }
    }
}

/// Canal do mixer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerDto {
    #[serde(default = "unity_gain")]
    pub volume: f32,
    #[serde(default = "center_pan")]
    pub pan: f32,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub soloed: bool,
    #[serde(default)]
    pub armed: bool,
    #[serde(default)]
    pub inserts: Vec<InsertDto>,
}

impl Default for MixerDto {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pan: 0.5,
            muted: false,
            soloed: false,
            armed: false,
            inserts: Vec::new(),
        }
    }
}

fn unity_gain() -> f32 {
    1.0
}

fn center_pan() -> f32 {
    0.5
}

/// Slot de insert (efeito) de um canal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertDto {
    pub name: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

fn enabled_default() -> bool {
    true
}

/// Clip posicionado na timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipDto {
    pub id: u64,
    pub track_id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: ClipKindDto,
    #[serde(default)]
    pub start_beat: f32,
    #[serde(default)]
    pub length_beats: f32,
    #[serde(default)]
    pub source_start_beats: f32,
    #[serde(default)]
    pub color: u8,
    #[serde(default)]
    pub color_override: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(default)]
    pub fade_in_beats: f32,
    #[serde(default)]
    pub fade_out_beats: f32,
    #[serde(default)]
    pub gain_db: f32,
    #[serde(default)]
    pub time_mode: TimeModeDto,
    #[serde(default)]
    pub warp_markers: Vec<WarpMarkerDto>,
    #[serde(default)]
    pub notes: Vec<NoteDto>,
}

/// Tipo de conteúdo do clip.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipKindDto {
    /// Clip de áudio.
    #[default]
    Audio,
    /// Clip MIDI.
    Midi,
}

impl From<ClipKind> for ClipKindDto {
    fn from(k: ClipKind) -> Self {
        match k {
            ClipKind::Audio => ClipKindDto::Audio,
            ClipKind::Midi => ClipKindDto::Midi,
        }
    }
}

impl From<ClipKindDto> for ClipKind {
    fn from(k: ClipKindDto) -> Self {
        match k {
            ClipKindDto::Audio => ClipKind::Audio,
            ClipKindDto::Midi => ClipKind::Midi,
        }
    }
}

/// Modo temporal do clip.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeModeDto {
    /// Musical (esticado, preserva pitch).
    Musical,
    /// Linear (velocidade natural).
    #[default]
    Linear,
}

impl From<TimeMode> for TimeModeDto {
    fn from(m: TimeMode) -> Self {
        match m {
            TimeMode::Musical => TimeModeDto::Musical,
            TimeMode::Linear => TimeModeDto::Linear,
        }
    }
}

impl From<TimeModeDto> for TimeMode {
    fn from(m: TimeModeDto) -> Self {
        match m {
            TimeModeDto::Musical => TimeMode::Musical,
            TimeModeDto::Linear => TimeMode::Linear,
        }
    }
}

/// Marcador de AudioWarp.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct WarpMarkerDto {
    pub source_seconds: f32,
    pub project_offset_beats: f32,
}

/// Nota MIDI.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NoteDto {
    #[serde(default)]
    pub id: i32,
    pub pitch: i32,
    pub start: f32,
    pub length: f32,
    #[serde(default = "default_velocity")]
    pub velocity: i32,
}

fn default_velocity() -> i32 {
    100
}

/// Mapa de tempo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempoMapDto {
    #[serde(default)]
    pub changes: Vec<TempoChangeDto>,
}

impl Default for TempoMapDto {
    fn default() -> Self {
        Self {
            changes: vec![TempoChangeDto {
                beat: 0.0,
                bpm: 120.0,
            }],
        }
    }
}

/// Mudança de BPM na timeline.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TempoChangeDto {
    pub beat: f32,
    pub bpm: f32,
}

/// Padrões do beat sequencer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeatPatternsDto {
    #[serde(default)]
    pub patterns: Vec<DrumPatternDto>,
    #[serde(default)]
    pub current: usize,
}

impl Default for BeatPatternsDto {
    fn default() -> Self {
        BeatPatternsDto::from(&BeatPatterns::new())
    }
}

/// Um padrão de bateria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrumPatternDto {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub grid: Vec<Vec<bool>>,
    #[serde(default = "default_resolution")]
    pub resolution: u32,
    #[serde(default)]
    pub track_params: Vec<DrumTrackParamsDto>,
    #[serde(default)]
    pub swing: f32,
    #[serde(default)]
    pub humanize: f32,
}

fn default_resolution() -> u32 {
    16
}

/// Parâmetros de uma trilha do padrão de bateria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrumTrackParamsDto {
    #[serde(default)]
    pub name: String,
    #[serde(default = "enabled_default")]
    pub active: bool,
    #[serde(default)]
    pub solo: bool,
    #[serde(default)]
    pub pan: f32,
    #[serde(default = "unity_gain")]
    pub volume: f32,
    #[serde(default)]
    pub mixer_channel: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_path: Option<String>,
}

/// Estado do transporte.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportDto {
    #[serde(default)]
    pub position_ticks: u64,
    #[serde(default)]
    pub play_start_ticks: u64,
    #[serde(default = "default_bpm")]
    pub bpm: u32,
    #[serde(default = "default_ppq")]
    pub ppq: u32,
    #[serde(default = "default_beats_per_bar")]
    pub beats_per_bar: u32,
    #[serde(default)]
    pub metronome_enabled: bool,
    #[serde(default)]
    pub metronome_sound: u8,
    #[serde(default = "unity_gain")]
    pub metronome_volume: f32,
    #[serde(default)]
    pub loop_enabled: bool,
    #[serde(default)]
    pub loop_start_ticks: u64,
    #[serde(default)]
    pub loop_end_ticks: u64,
    #[serde(default)]
    pub beat_only: bool,
}

fn default_bpm() -> u32 {
    120
}

fn default_ppq() -> u32 {
    960
}

fn default_beats_per_bar() -> u32 {
    4
}

// =============================================================================
// Conversão daw-core <-> DTO
// =============================================================================

impl ProjectFile {
    /// Monta o documento a partir do projeto em memória.
    pub fn from_project(project: &Project) -> Self {
        Self::build(project, None)
    }

    /// Monta o documento incluindo o transporte (playhead, loop).
    pub fn from_project_with_transport(project: &Project, transport: &Transport) -> Self {
        Self::build(project, Some(transport))
    }

    fn build(project: &Project, transport: Option<&Transport>) -> Self {
        Self {
            version: FORMAT_VERSION,
            name: String::new(),
            tracks: project.tracks.iter().map(TrackDto::from).collect(),
            clips: project.clips.iter().map(ClipDto::from).collect(),
            tempo_map: TempoMapDto::from(&project.tempo_map),
            master_mixer: MixerDto::from(&project.master_mixer),
            beat_patterns: BeatPatternsDto::from(&project.beat_patterns),
            transport: transport.map(TransportDto::from),
        }
    }

    /// Reconstrói o projeto em memória.
    ///
    /// `History` e subscribers nascem vazios: undo/redo não cruza a
    /// fronteira de um arquivo salvo.
    pub fn to_project(&self) -> Project {
        let mut project = Project::default();
        project.tracks = self.tracks.iter().map(Track::from).collect();
        project.clips = self.clips.iter().map(Clip::from).collect();
        project.tempo_map = self.tempo_map.to_tempo_map();
        project.master_mixer = self.master_mixer.to_channel();
        project.beat_patterns = self.beat_patterns.to_beat_patterns();
        project
    }

    /// Aplica o transporte salvo sobre um `Transport` existente (a UI é
    /// dona da instância; o arquivo só carrega os valores).
    pub fn apply_transport(&self, transport: &mut Transport) {
        if let Some(dto) = &self.transport {
            dto.apply_to(transport);
        }
    }

    /// Serializa em JSON legível.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Lê do JSON, recusando versões futuras.
    pub fn from_json(text: &str) -> Result<Self> {
        let file: ProjectFile = serde_json::from_str(text)?;
        if file.version > FORMAT_VERSION {
            return Err(ProjectError::UnsupportedVersion {
                found: file.version,
                supported: FORMAT_VERSION,
            });
        }
        Ok(file)
    }

    /// Grava o projeto no caminho indicado (cria os diretórios pais).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, self.to_json()?)?;
        Ok(())
    }

    /// Lê um projeto do disco.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::from_json(&text)
    }
}

impl From<&Track> for TrackDto {
    fn from(t: &Track) -> Self {
        Self {
            id: t.id,
            name: t.name.clone(),
            kind: TrackKindDto::from(t.kind),
            color: t.color,
            mixer: MixerDto::from(&t.mixer),
        }
    }
}

impl From<&TrackDto> for Track {
    fn from(d: &TrackDto) -> Self {
        Self {
            id: d.id,
            name: d.name.clone(),
            kind: TrackKind::from(d.kind),
            color: d.color,
            mixer: d.mixer.to_channel(),
        }
    }
}

impl From<&MixerChannel> for MixerDto {
    fn from(m: &MixerChannel) -> Self {
        Self {
            volume: m.volume,
            pan: m.pan,
            muted: m.muted,
            soloed: m.soloed,
            armed: m.armed,
            inserts: m
                .inserts
                .iter()
                .map(|s| InsertDto {
                    name: s.name.clone(),
                    enabled: s.enabled,
                })
                .collect(),
        }
    }
}

impl MixerDto {
    fn to_channel(&self) -> MixerChannel {
        MixerChannel {
            volume: self.volume,
            pan: self.pan,
            muted: self.muted,
            soloed: self.soloed,
            armed: self.armed,
            inserts: self
                .inserts
                .iter()
                .map(|s| InsertSlot {
                    name: s.name.clone(),
                    enabled: s.enabled,
                })
                .collect(),
        }
    }
}

impl From<&Clip> for ClipDto {
    fn from(c: &Clip) -> Self {
        Self {
            id: c.id,
            track_id: c.track_id,
            name: c.name.clone(),
            kind: ClipKindDto::from(c.kind),
            start_beat: c.start_beat,
            length_beats: c.length_beats,
            source_start_beats: c.source_start_beats,
            color: c.color,
            color_override: c.color_override,
            file_path: c.file_path.clone(),
            fade_in_beats: c.fade_in_beats,
            fade_out_beats: c.fade_out_beats,
            gain_db: c.gain_db,
            time_mode: TimeModeDto::from(c.time_mode),
            warp_markers: c
                .warp_markers
                .iter()
                .map(|m| WarpMarkerDto {
                    source_seconds: m.source_seconds,
                    project_offset_beats: m.project_offset_beats,
                })
                .collect(),
            notes: c
                .notes
                .iter()
                .map(|n| NoteDto {
                    id: n.id,
                    pitch: n.pitch,
                    start: n.start,
                    length: n.length,
                    velocity: n.velocity,
                })
                .collect(),
        }
    }
}

impl From<&ClipDto> for Clip {
    fn from(d: &ClipDto) -> Self {
        Self {
            id: d.id,
            track_id: d.track_id,
            name: d.name.clone(),
            kind: ClipKind::from(d.kind),
            start_beat: d.start_beat,
            length_beats: d.length_beats,
            source_start_beats: d.source_start_beats,
            color: d.color,
            color_override: d.color_override,
            file_path: d.file_path.clone(),
            fade_in_beats: d.fade_in_beats,
            fade_out_beats: d.fade_out_beats,
            gain_db: d.gain_db,
            time_mode: TimeMode::from(d.time_mode),
            warp_markers: d
                .warp_markers
                .iter()
                .map(|m| WarpMarker {
                    source_seconds: m.source_seconds,
                    project_offset_beats: m.project_offset_beats,
                })
                .collect(),
            notes: d
                .notes
                .iter()
                .map(|n| daw_midi::Note {
                    id: n.id,
                    pitch: n.pitch,
                    start: n.start,
                    length: n.length,
                    velocity: n.velocity,
                })
                .collect(),
        }
    }
}

impl From<&TempoMap> for TempoMapDto {
    fn from(m: &TempoMap) -> Self {
        Self {
            changes: m
                .changes
                .iter()
                .map(|c| TempoChangeDto {
                    beat: c.beat,
                    bpm: c.bpm,
                })
                .collect(),
        }
    }
}

impl TempoMapDto {
    fn to_tempo_map(&self) -> TempoMap {
        let mut changes: Vec<TempoChange> = self
            .changes
            .iter()
            .map(|c| TempoChange {
                beat: c.beat,
                bpm: c.bpm,
            })
            .collect();
        if changes.is_empty() {
            changes.push(TempoChange {
                beat: 0.0,
                bpm: 120.0,
            });
        }
        TempoMap { changes }
    }
}

impl From<&BeatPatterns> for BeatPatternsDto {
    fn from(b: &BeatPatterns) -> Self {
        Self {
            patterns: b.patterns.iter().map(DrumPatternDto::from).collect(),
            current: b.current,
        }
    }
}

impl BeatPatternsDto {
    fn to_beat_patterns(&self) -> BeatPatterns {
        if self.patterns.is_empty() {
            return BeatPatterns::new();
        }
        BeatPatterns {
            patterns: self.patterns.iter().map(DrumPattern::from).collect(),
            current: self.current.min(self.patterns.len() - 1),
        }
    }
}

impl From<&DrumPattern> for DrumPatternDto {
    fn from(p: &DrumPattern) -> Self {
        Self {
            name: p.name.clone(),
            grid: p.grid.clone(),
            resolution: p.resolution,
            track_params: p
                .track_params
                .iter()
                .map(DrumTrackParamsDto::from)
                .collect(),
            swing: p.swing,
            humanize: p.humanize,
        }
    }
}

impl From<&DrumPatternDto> for DrumPattern {
    fn from(d: &DrumPatternDto) -> Self {
        Self {
            name: d.name.clone(),
            grid: d.grid.clone(),
            resolution: d.resolution,
            track_params: d
                .track_params
                .iter()
                .map(DrumTrackParams::from)
                .collect(),
            swing: d.swing,
            humanize: d.humanize,
        }
    }
}

impl From<&DrumTrackParams> for DrumTrackParamsDto {
    fn from(p: &DrumTrackParams) -> Self {
        Self {
            name: p.name.clone(),
            active: p.active,
            solo: p.solo,
            pan: p.pan,
            volume: p.volume,
            mixer_channel: p.mixer_channel,
            sample_path: p.sample_path.clone(),
        }
    }
}

impl From<&DrumTrackParamsDto> for DrumTrackParams {
    fn from(d: &DrumTrackParamsDto) -> Self {
        Self {
            name: d.name.clone(),
            active: d.active,
            solo: d.solo,
            pan: d.pan,
            volume: d.volume,
            mixer_channel: d.mixer_channel,
            sample_path: d.sample_path.clone(),
        }
    }
}

impl From<&Transport> for TransportDto {
    fn from(t: &Transport) -> Self {
        Self {
            position_ticks: t.position_ticks,
            play_start_ticks: t.play_start_ticks,
            bpm: t.bpm,
            ppq: t.ppq,
            beats_per_bar: t.beats_per_bar,
            metronome_enabled: t.metronome_enabled,
            metronome_sound: t.metronome_sound,
            metronome_volume: t.metronome_volume,
            loop_enabled: t.loop_enabled,
            loop_start_ticks: t.loop_start_ticks,
            loop_end_ticks: t.loop_end_ticks,
            beat_only: t.mode == PlayMode::BeatOnly,
        }
    }
}

impl TransportDto {
    fn apply_to(&self, t: &mut Transport) {
        t.position_ticks = self.position_ticks;
        t.play_start_ticks = self.play_start_ticks;
        t.bpm = self.bpm;
        t.ppq = self.ppq;
        t.beats_per_bar = self.beats_per_bar;
        t.metronome_enabled = self.metronome_enabled;
        t.metronome_sound = self.metronome_sound;
        t.metronome_volume = self.metronome_volume;
        t.loop_enabled = self.loop_enabled;
        t.loop_start_ticks = self.loop_start_ticks;
        t.loop_end_ticks = self.loop_end_ticks;
        t.mode = if self.beat_only {
            PlayMode::BeatOnly
        } else {
            PlayMode::Timeline
        };
        // O arquivo nunca reabre tocando.
        t.playing = false;
    }
}

// =============================================================================
// Testes
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_demo_preserva_estrutura() {
        let project = Project::demo();
        let transport = Transport {
            position_ticks: 1234,
            loop_enabled: true,
            loop_start_ticks: 960,
            loop_end_ticks: 3840,
            ..Transport::default()
        };

        let file = ProjectFile::from_project_with_transport(&project, &transport);
        let json = file.to_json().expect("serializar");
        let back = ProjectFile::from_json(&json).expect("desserializar");
        let restored = back.to_project();

        assert_eq!(restored.tracks.len(), project.tracks.len());
        assert_eq!(restored.clips.len(), project.clips.len());
        assert_eq!(restored.tempo_map.changes.len(), project.tempo_map.changes.len());
        assert_eq!(restored.beat_patterns.patterns.len(), project.beat_patterns.patterns.len());

        for (a, b) in project.tracks.iter().zip(restored.tracks.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.name, b.name);
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.color, b.color);
            assert_eq!(a.mixer.volume, b.mixer.volume);
            assert_eq!(a.mixer.inserts.len(), b.mixer.inserts.len());
        }

        for (a, b) in project.clips.iter().zip(restored.clips.iter()) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.track_id, b.track_id);
            assert_eq!(a.kind, b.kind);
            assert_eq!(a.start_beat, b.start_beat);
            assert_eq!(a.length_beats, b.length_beats);
            assert_eq!(a.notes.len(), b.notes.len());
            assert_eq!(a.warp_markers.len(), b.warp_markers.len());
        }

        assert_eq!(project.master_mixer.volume, restored.master_mixer.volume);
    }

    #[test]
    fn transporte_volta_como_parado() {
        let project = Project::demo();
        let transport = Transport {
            playing: true,
            position_ticks: 777,
            loop_enabled: true,
            ..Transport::default()
        };
        let file = ProjectFile::from_project_with_transport(&project, &transport);
        let mut restored = Transport::default();
        file.apply_transport(&mut restored);

        assert_eq!(restored.position_ticks, 777);
        assert!(restored.loop_enabled);
        assert!(!restored.playing, "projeto reabre parado");
    }

    #[test]
    fn versao_futura_e_recusada() {
        let json = r#"{"version": 99, "tracks": [], "clips": []}"#;
        match ProjectFile::from_json(json) {
            Err(ProjectError::UnsupportedVersion { found, supported }) => {
                assert_eq!(found, 99);
                assert_eq!(supported, FORMAT_VERSION);
            }
            other => panic!("esperava UnsupportedVersion, veio {other:?}"),
        }
    }

    #[test]
    fn campos_ausentes_usam_default() {
        let json = r#"{"tracks":[{"id":7,"name":"Vox"}]}"#;
        let file = ProjectFile::from_json(json).expect("abrir antigo");
        assert_eq!(file.version, FORMAT_VERSION);
        let project = file.to_project();
        assert_eq!(project.tracks.len(), 1);
        assert_eq!(project.tracks[0].kind, TrackKind::Audio);
        assert_eq!(project.tracks[0].mixer.volume, 1.0);
        assert_eq!(project.tempo_map.changes[0].bpm, 120.0);
    }

    #[test]
    fn salva_e_carrega_do_disco() {
        let project = Project::demo();
        let dir = std::env::temp_dir().join("ruana-project-test");
        let path = dir.join("roundtrip.ruana");

        let file = ProjectFile::from_project(&project);
        file.save(&path).expect("salvar");
        assert!(path.exists(), "arquivo criado");

        let loaded = ProjectFile::load(&path).expect("carregar");
        assert_eq!(loaded.tracks.len(), project.tracks.len());
        assert_eq!(loaded.clips.len(), project.clips.len());

        let _ = std::fs::remove_file(&path);
    }
}
