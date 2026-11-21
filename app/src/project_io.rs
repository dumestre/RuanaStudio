//! Arquivo de projeto: caminho atual, flag de alterações e gravação.
//!
//! A serialização vive em `daw-project`; aqui só mora o **estado de UI**
//! (qual arquivo está aberto, se há mudanças não salvas) e o diálogo nativo.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use daw_core::project::Project;
use daw_core::transport::Transport;
use daw_project::{FILE_EXTENSION, ProjectFile};

/// Nome mostrado quando o projeto ainda não foi salvo.
pub const UNTITLED: &str = "Sem titulo";

/// Estado de arquivo do documento aberto.
pub struct ProjectIo {
    path: Option<PathBuf>,
    /// Compartilhado com o subscriber do `Project` (ver `host::EngineBridge`).
    dirty: Rc<Cell<bool>>,
}

impl ProjectIo {
    /// Cria o estado vazio (projeto novo, sem arquivo).
    pub fn new() -> Self {
        Self {
            path: None,
            dirty: Rc::new(Cell::new(false)),
        }
    }

    /// Handle da flag para o subscriber de eventos do documento.
    pub fn dirty_flag(&self) -> Rc<Cell<bool>> {
        self.dirty.clone()
    }

    /// Caminho do arquivo aberto, se houver.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// `true` quando há mudanças desde o último save.
    pub fn is_dirty(&self) -> bool {
        self.dirty.get()
    }

    /// Marca o documento como alterado.
    pub fn mark_dirty(&self) {
        self.dirty.set(true);
    }

    /// Limpa a flag (depois de gravar ou abrir).
    pub fn clear_dirty(&self) {
        self.dirty.set(false);
    }

    /// Define o arquivo aberto (após "Salvar como" ou abrir).
    pub fn set_path(&mut self, path: PathBuf) {
        self.path = Some(path);
    }

    /// Nome curto para a barra de título.
    pub fn display_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| UNTITLED.to_string())
    }

    /// Título da janela, com `*` quando há alterações pendentes.
    pub fn window_title(&self) -> String {
        let mark = if self.is_dirty() { " *" } else { "" };
        format!("{}{} — Ruana Studio", self.display_name(), mark)
    }
}

impl Default for ProjectIo {
    fn default() -> Self {
        Self::new()
    }
}

/// Grava o projeto no disco.
///
/// Sempre que `force_dialog` for `true` (ou não houver caminho atual) abre o
/// diálogo "Salvar como"; caso contrário grava por cima do arquivo aberto.
pub fn save_project(
    io: &mut ProjectIo,
    project: &Project,
    transport: &Transport,
    force_dialog: bool,
) -> Result<PathBuf, String> {
    let target = if force_dialog || io.path.is_none() {
        choose_save_path(io.path.as_deref()).ok_or_else(|| "salvar cancelado".to_string())?
    } else {
        io.path.clone().expect("caminho conferido acima")
    };

    let file = ProjectFile::from_project_with_transport(project, transport);
    file.save(&target)
        .map_err(|e| format!("falha ao salvar {}: {e}", target.display()))?;

    io.path = Some(target.clone());
    io.clear_dirty();
    Ok(target)
}

/// Diálogo nativo "Salvar como". Devolve `None` se o usuário cancelar.
#[cfg(target_os = "windows")]
pub fn choose_save_path(current: Option<&Path>) -> Option<PathBuf> {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetSaveFileNameW, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows_sys::core::{PCWSTR, PWSTR};

    let filter: Vec<u16> = "Ruana Project\0*.ruana\0All Files\0*.*\0\0"
        .encode_utf16()
        .collect();
    let ext: Vec<u16> = format!("{FILE_EXTENSION}\0").encode_utf16().collect();

    let mut path = vec![0u16; 32768];
    if let Some(p) = current {
        let text: Vec<u16> = p.to_string_lossy().encode_utf16().take(32766).collect();
        path[..text.len()].copy_from_slice(&text);
    }

    let mut dialog = OPENFILENAMEW::default();
    dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = HWND::default();
    dialog.lpstrFilter = PCWSTR::from(filter.as_ptr());
    dialog.lpstrFile = PWSTR::from(path.as_mut_ptr());
    dialog.nMaxFile = path.len() as u32;
    dialog.lpstrDefExt = PCWSTR::from(ext.as_ptr());
    dialog.Flags = OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST;

    if unsafe { GetSaveFileNameW(&mut dialog) } != 0 {
        let len = path.iter().position(|c| *c == 0).unwrap_or(0);
        Some(PathBuf::from(String::from_utf16_lossy(&path[..len])))
    } else {
        None
    }
}

/// Fallback (Linux/macOS): usa `rfd`, como no seletor de áudio.
#[cfg(not(target_os = "windows"))]
pub fn choose_save_path(current: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new()
        .add_filter("Ruana Project", &[FILE_EXTENSION])
        .set_file_name(
            current
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| format!("untitled.{FILE_EXTENSION}")),
        );
    if let Some(dir) = current.and_then(|p| p.parent()) {
        dialog = dialog.set_directory(dir);
    }
    dialog.save_file()
}

/// Diálogo nativo "Abrir". Devolve `None` se o usuário cancelar.
#[cfg(target_os = "windows")]
pub fn choose_open_path() -> Option<PathBuf> {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    use windows_sys::core::{PCWSTR, PWSTR};

    let filter: Vec<u16> = "Ruana Project\0*.ruana\0All Files\0*.*\0\0"
        .encode_utf16()
        .collect();
    let mut path = vec![0u16; 32768];

    let mut dialog = OPENFILENAMEW::default();
    dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = HWND::default();
    dialog.lpstrFilter = PCWSTR::from(filter.as_ptr());
    dialog.lpstrFile = PWSTR::from(path.as_mut_ptr());
    dialog.nMaxFile = path.len() as u32;
    dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST;

    if unsafe { GetOpenFileNameW(&mut dialog) } != 0 {
        let len = path.iter().position(|c| *c == 0).unwrap_or(0);
        Some(PathBuf::from(String::from_utf16_lossy(&path[..len])))
    } else {
        None
    }
}

/// Fallback (Linux/macOS).
#[cfg(not(target_os = "windows"))]
pub fn choose_open_path() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Ruana Project", &[FILE_EXTENSION])
        .pick_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titulo_reflete_pendencias() {
        let io = ProjectIo::new();
        assert_eq!(io.display_name(), UNTITLED);
        assert!(!io.is_dirty());
        io.mark_dirty();
        assert!(io.window_title().ends_with(" * — Ruana Studio"));
        io.clear_dirty();
        assert!(!io.is_dirty());
    }
}
