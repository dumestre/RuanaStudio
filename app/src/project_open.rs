//! Abertura de projeto: substitui o documento e re-projeta tudo na UI.
//!
//! Abrir não é só trocar o `Project`: a UI guarda modelos (`VecModel`) e
//! caches derivados do documento antigo. Todos precisam ser atualizados **no
//! lugar** (sem recriar os `Rc`), senão as janelas flutuantes e os handlers
//! de evento continuam escrevendo em buffers órfãos.
//!
//! O espelho de clips da engine wgpu não é tocado aqui: ele é reconstruído no
//! próximo `tick` a partir do `clips_flag` (ver `host::EngineBridge::tick`).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use daw_core::project::Project;
use daw_core::transport::Transport;
use daw_project::ProjectFile;

use crate::mixer::{mixer_inserts_state, replace_rows, MixerModels};
use crate::models::bars_for_end_beat;
use crate::waveform::{to_clip_data, WaveformCache};
use crate::{ClipData, MainWindow};

/// Aplica um documento recém-carregado na UI.
///
/// Devolve `Err` com a mensagem para o `eprintln` do chamador; em caso de
/// erro o estado anterior é mantido (validação acontece antes de mutar).
pub fn apply(
    ui: &MainWindow,
    file: &ProjectFile,
    project: &Rc<RefCell<Project>>,
    transport: &Arc<Mutex<Transport>>,    waveforms: &Rc<RefCell<WaveformCache>>,
    tracks_model: &Rc<slint::VecModel<slint::SharedString>>,
    track_colors_model: &Rc<slint::VecModel<i32>>,
    clips_model: &Rc<slint::VecModel<ClipData>>,
    mixer_models: &Rc<RefCell<MixerModels>>,
) -> Result<(), String> {
    // 1. Transporte antes do documento: o `tick` lê bpm do transporte para
    //    re-espelhar os clips, então ele precisa estar correto primeiro.
    let (bpm, ppq, bpb) = {
        let mut t = transport
            .lock()
            .map_err(|_| "transporte indisponível".to_string())?;
        file.apply_transport(&mut t);
        t.playing = false;
        (t.bpm as i32, t.ppq as i32, t.beats_per_bar as i32)
    };

    // 2. Cache de waveform: os ids vieram do arquivo, não do cache atual.
    waveforms.borrow_mut().clear();

    // 3. Documento.
    let new_project = file.to_project();
    *project.borrow_mut() = new_project;

    // 4. Modelos derivados (mutados no lugar, `Rc` preservado).
    let (names, colors, clips, inserts, total_bars) = {
        let proj = project.borrow();
        let names: Vec<slint::SharedString> =
            proj.tracks.iter().map(|t| t.name.clone().into()).collect();
        let colors: Vec<i32> = proj.tracks.iter().map(|t| t.color as i32).collect();
        let waves = waveforms.borrow();
        let clips: Vec<ClipData> = proj.clips.iter().map(|c| to_clip_data(c, &waves)).collect();
        let end = proj
            .clips
            .iter()
            .map(|c| c.end_beat())
            .fold(0.0f32, f32::max);
        let inserts = mixer_inserts_state(&proj);
        let bars = bars_for_end_beat(end as f64, bpb);
        (names, colors, clips, inserts, bars)
    };

    replace_rows(tracks_model, &names);
    replace_rows(track_colors_model, &colors);
    replace_rows(clips_model, &clips);

    let (insert_counts, insert_names, insert_enabled, max_inserts) = inserts;
    ui.set_mixer_insert_counts(Rc::new(slint::VecModel::from(insert_counts)).into());
    ui.set_mixer_insert_names(Rc::new(slint::VecModel::from(insert_names)).into());
    ui.set_mixer_insert_enabled(Rc::new(slint::VecModel::from(insert_enabled)).into());
    ui.set_mixer_max_inserts(max_inserts);

    mixer_models.borrow().reset_from_project(&project.borrow());
    crate::mixer::apply_mixer_models!(ui, &mixer_models.borrow());

    // 5. Métrica e tamanho da timeline.
    ui.set_bpm(bpm);
    ui.set_ppq(ppq);
    ui.set_beats_per_bar(bpb);
    ui.set_total_bars(total_bars);

    Ok(())
}
