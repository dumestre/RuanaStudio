slint::include_modules!();

mod backend;

use backend::versions;
use std::path::Path;
use std::path::PathBuf;

fn main() -> Result<(), slint::PlatformError> {
    let ui = HubWindow::new()?;
    let paths = match backend::HubPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            ui.set_status(format!("Não foi possível inicializar o Hub: {error}").into());
            ui.run()?;
            return Ok(());
        }
    };

    if let Err(error) = paths.ensure() {
        ui.set_status(format!("Falha ao preparar os diretórios do Hub: {error}").into());
    }

    refresh_all(&ui, &paths.versions_dir);

    {
        let ui_weak = ui.as_weak();
        let versions_dir = paths.versions_dir.clone();
        ui.on_install_test_version_requested(move || {
            start_channel_install(ui_weak.clone(), versions_dir.clone(), None);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let versions_dir = paths.versions_dir.clone();
        ui.on_update_test_version_requested(move |installed| {
            start_channel_install(
                ui_weak.clone(),
                versions_dir.clone(),
                Some(PathBuf::from(installed.as_str())),
            );
        });
    }

    {
        let ui_weak = ui.as_weak();
        let versions_dir = paths.versions_dir.clone();
        ui.on_install_github_requested(move || {
            start_github_install(ui_weak.clone(), versions_dir.clone(), None);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let versions_dir = paths.versions_dir.clone();
        ui.on_update_github_requested(move || {
            start_github_install(ui_weak.clone(), versions_dir.clone(), None);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let versions_dir = paths.versions_dir.clone();
        ui.on_install_github_tag_requested(move || {
            start_github_tag_install(ui_weak.clone(), versions_dir.clone(), None);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let versions_dir = paths.versions_dir.clone();
        ui.on_update_github_tag_requested(move || {
            start_github_tag_install(ui_weak.clone(), versions_dir.clone(), None);
        });
    }

    {
        let ui_weak = ui.as_weak();
        ui.on_launch_version_requested(move |path| {
            match versions::launch(Path::new(path.as_str())) {
                Ok(()) => set_status(&ui_weak, "Ruana Studio iniciado.".to_owned()),
                Err(error) => set_status(
                    &ui_weak,
                    format!("Não foi possível iniciar a versão: {error}"),
                ),
            }
        });
    }

    {
        let ui_weak = ui.as_weak();
        let versions_dir = paths.versions_dir.clone();
        ui.on_source_changed(move |source| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_selected_source(source);
                refresh_all(&ui, &versions_dir);
            }
        });
    }

    ui.run()
}

fn set_status(ui: &slint::Weak<HubWindow>, message: String) {
    if let Some(ui) = ui.upgrade() {
        ui.set_status(message.into());
    }
}

fn start_channel_install(
    ui: slint::Weak<HubWindow>,
    versions_dir: PathBuf,
    installed: Option<PathBuf>,
) {
    if let Some(window) = ui.upgrade() {
        if window.get_busy() {
            return;
        }
        window.set_busy(true);
    } else {
        return;
    }
    let operation = if installed.is_some() {
        "Baixando atualização do canal..."
    } else {
        "Consultando o canal e baixando a versão..."
    };
    set_status(&ui, operation.to_owned());

    let ui_weak = ui.clone();
    let worker_versions_dir = versions_dir.clone();
    let spawn = std::thread::Builder::new()
        .name("ruana-hub-updater".to_owned())
        .spawn(move || {
            let result = match installed {
                Some(installed) => versions::update_latest(&worker_versions_dir, &installed),
                None => versions::install_latest(&worker_versions_dir),
            };
            let status = match &result {
                Ok(path) => format!("Build do canal instalada: {}", path.display()),
                Err(error) => format!("Falha na instalação da build: {error}"),
            };

            let dispatch = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    refresh_all(&ui, &worker_versions_dir);
                    ui.set_status(status.into());
                    ui.set_busy(false);
                }
            });
            if let Err(error) = dispatch {
                eprintln!(
                    "[ruana-hub] não foi possível publicar o resultado na interface: {error}"
                );
            }
        });
    if let Err(error) = spawn {
        set_status(&ui, format!("Não foi possível iniciar o download: {error}"));
        if let Some(window) = ui.upgrade() {
            window.set_busy(false);
        }
    }
}

fn start_github_install(
    ui: slint::Weak<HubWindow>,
    versions_dir: PathBuf,
    _installed: Option<PathBuf>,
) {
    if let Some(window) = ui.upgrade() {
        if window.get_busy() {
            return;
        }
        window.set_busy(true);
    } else {
        return;
    }
    let operation = "Consultando GitHub Releases e baixando...";
    set_status(&ui, operation.to_owned());

    let ui_weak = ui.clone();
    let worker_versions_dir = versions_dir.clone();
    let spawn = std::thread::Builder::new()
        .name("ruana-hub-github-updater".to_owned())
        .spawn(move || {
            let result = versions::install_latest_github(&worker_versions_dir);
            let status = match &result {
                Ok(path) => format!("Release do GitHub instalada: {}", path.display()),
                Err(error) => format!("Falha na instalação do release: {error}"),
            };

            let dispatch = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    refresh_all(&ui, &worker_versions_dir);
                    ui.set_status(status.into());
                    ui.set_busy(false);
                }
            });
            if let Err(error) = dispatch {
                eprintln!(
                    "[ruana-hub] não foi possível publicar o resultado na interface: {error}"
                );
            }
        });
    if let Err(error) = spawn {
        set_status(&ui, format!("Não foi possível iniciar o download: {error}"));
        if let Some(window) = ui.upgrade() {
            window.set_busy(false);
        }
    }
}

fn start_github_tag_install(
    ui: slint::Weak<HubWindow>,
    versions_dir: PathBuf,
    _installed: Option<PathBuf>,
) {
    if let Some(window) = ui.upgrade() {
        if window.get_busy() {
            return;
        }
        window.set_busy(true);
    } else {
        return;
    }
    let operation = "Consultando tags do GitHub e baixando...";
    set_status(&ui, operation.to_owned());

    let ui_weak = ui.clone();
    let worker_versions_dir = versions_dir.clone();
    let spawn = std::thread::Builder::new()
        .name("ruana-hub-github-tag-updater".to_owned())
        .spawn(move || {
            let result = versions::install_latest_github_tag(&worker_versions_dir);
            let status = match &result {
                Ok(path) => format!("Tag do GitHub instalada: {}", path.display()),
                Err(error) => format!("Falha na instalação da tag: {error}"),
            };

            let dispatch = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    refresh_all(&ui, &worker_versions_dir);
                    ui.set_status(status.into());
                    ui.set_busy(false);
                }
            });
            if let Err(error) = dispatch {
                eprintln!(
                    "[ruana-hub] não foi possível publicar o resultado na interface: {error}"
                );
            }
        });
    if let Err(error) = spawn {
        set_status(&ui, format!("Não foi possível iniciar o download: {error}"));
        if let Some(window) = ui.upgrade() {
            window.set_busy(false);
        }
    }
}

fn refresh_all(ui: &HubWindow, root: &Path) {
    refresh_versions(ui, root);
    refresh_github_releases(ui);
    refresh_github_tags(ui);
}

fn refresh_versions(ui: &HubWindow, root: &Path) {
    match versions::list(root) {
        Ok(items) => {
            ui.set_versions(
                std::rc::Rc::new(slint::VecModel::from(
                    items
                        .into_iter()
                        .map(|path| HubVersion {
                            name: path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("Ruana Studio")
                                .into(),
                            path: path.to_string_lossy().into_owned().into(),
                            source: "Canal (R2)".into(),
                        })
                        .collect::<Vec<_>>(),
                ))
                .into(),
            );
        }
        Err(error) => {
            ui.set_status(format!("Não foi possível carregar as versões: {error}").into())
        }
    }
}

fn refresh_github_releases(ui: &HubWindow) {
    match versions::check_github_releases() {
        Ok(updates) => {
            let releases: Vec<GitHubReleaseInfo> = updates
                .into_iter()
                .take(5)
                .map(|u| GitHubReleaseInfo {
                    version: u.version.into(),
                    release_notes: truncate_notes(&u.release_notes, 200).into(),
                    asset_size: format_bytes(u.asset_size).into(),
                })
                .collect();
            ui.set_github_releases(
                std::rc::Rc::new(slint::VecModel::from(releases)).into(),
            );
        }
        Err(error) => {
            ui.set_status(format!("Erro ao buscar GitHub Releases: {error}").into());
            ui.set_github_releases(std::rc::Rc::new(slint::VecModel::from(Vec::<GitHubReleaseInfo>::new())).into());
        }
    }
}

fn refresh_github_tags(ui: &HubWindow) {
    match versions::check_github_tags() {
        Ok(updates) => {
            let tags: Vec<GitHubTagInfo> = updates
                .into_iter()
                .take(5)
                .map(|u| GitHubTagInfo {
                    version: u.version.into(),
                    release_notes: truncate_notes(&u.release_notes, 200).into(),
                    asset_size: format_bytes(u.asset_size).into(),
                })
                .collect();
            ui.set_github_tags(
                std::rc::Rc::new(slint::VecModel::from(tags)).into(),
            );
        }
        Err(error) => {
            ui.set_status(format!("Erro ao buscar tags do GitHub: {error}").into());
            ui.set_github_tags(std::rc::Rc::new(slint::VecModel::from(Vec::<GitHubTagInfo>::new())).into());
        }
    }
}

fn truncate_notes(notes: &str, max_len: usize) -> String {
    let cleaned = notes.lines().take(3).collect::<Vec<_>>().join(" ");
    if cleaned.len() > max_len {
        format!("{}...", &cleaned[..max_len])
    } else {
        cleaned
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
