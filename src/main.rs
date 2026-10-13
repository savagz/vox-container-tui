mod app;
mod config;
mod deps;
mod event;
mod i18n;
mod keys;
mod logging;
mod lxc;
mod setup;
mod sysmon;
mod theme;
mod ui;

use std::time::Duration;

use app::{App, PendingAction, Reload};
use color_eyre::Result;
use config::Config;
use crossterm::event::{Event, EventStream};
use event::AppEvent;
use futures_util::StreamExt;
use i18n::{Key, Lang};
use lxc::cli::Cli;
use lxc::Backend;
use setup::{SystemCheckMode, SystemCheckOutcome, SystemCheckState};
use theme::{Status, Theme};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

const USAGE: &str = "voxcontainer-tui: TUI para administrar instancias LXD/Incus

USO:
    voxcontainer-tui [OPCIONES]

OPCIONES:
    -V, --version    Muestra la versión y sale
    -h, --help       Muestra esta ayuda y sale

ENTORNO:
    VOXCONTAINER_FIXTURES=1    Fuerza el modo fixtures (datos falsos, sin daemon)";

/// Atiende `--version`/`--help` antes de tocar la terminal: así `install.sh`
/// puede comprobar la instalación sin abrir la TUI. Devuelve `true` si ya
/// respondió y hay que salir.
fn handle_cli_args() -> bool {
    match std::env::args().nth(1).as_deref() {
        Some("-V" | "--version") => {
            println!("voxcontainer-tui {}", env!("CARGO_PKG_VERSION"));
            true
        }
        Some("-h" | "--help") => {
            println!("{USAGE}");
            true
        }
        _ => false,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if handle_cli_args() {
        return Ok(());
    }
    color_eyre::install()?;
    let _log_guard = logging::init()?;

    let mut terminal = ratatui::init();
    let result = run(&mut terminal).await;
    ratatui::restore();
    result
}

async fn run(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let (tx, mut rx) = mpsc::channel::<AppEvent>(100);
    let mut input = Input::new(tx.clone());
    input.start();
    spawn_signal_task(tx.clone());

    // La config se carga antes que nada (aunque el tablero recién exista después
    // del gate) porque el idioma elegido tiene que aplicarse ya al wizard de
    // verificación, no sólo al tablero.
    let config = Config::load().ok().flatten().unwrap_or_default();

    // La ventana de verificación bloquea el paso al tablero mientras haya
    // chequeos bloqueantes en rojo (binario, daemon, permiso de socket) — a
    // diferencia de otras TUIs de la familia VOXPBX, acá no hay credenciales que
    // pedir: lo único que puede impedir arrancar es que LXD/Incus no esté listo.
    let mut system_check = SystemCheckState::new(config.ui.language);
    if !system_check.report.all_blocking_ok() {
        match run_gate(terminal, &mut rx, &mut input, &mut system_check).await? {
            GateResult::Continue => {}
            GateResult::Quit => return Ok(()),
        }
    }

    run_app(terminal, config, system_check, tx, rx, input).await
}

/// La tarea que lee el teclado tiene que poder pararse: mientras corre `sudo` (o
/// `lxc exec`) la terminal es del comando, y dos lectores compitiendo por el
/// mismo tty se comen las teclas del usuario (incluida la contraseña).
struct Input {
    tx: mpsc::Sender<AppEvent>,
    handle: Option<JoinHandle<()>>,
}

impl Input {
    fn new(tx: mpsc::Sender<AppEvent>) -> Self {
        Self { tx, handle: None }
    }

    fn start(&mut self) {
        if self.handle.is_none() {
            self.handle = Some(spawn_input_task(self.tx.clone()));
        }
    }

    fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

enum GateResult {
    Continue,
    Quit,
}

/// Wizard a pantalla completa: se queda acá hasta que el usuario resuelva los
/// chequeos bloqueantes, decida avanzar igual (`a`), o cancele (`Esc`/`q`).
async fn run_gate(
    terminal: &mut ratatui::DefaultTerminal,
    rx: &mut mpsc::Receiver<AppEvent>,
    input: &mut Input,
    state: &mut SystemCheckState,
) -> Result<GateResult> {
    let theme = Theme::default();
    terminal.draw(|frame| ui::setup::render_gate(frame, frame.area(), state, &theme))?;

    while let Some(event) = rx.recv().await {
        match event {
            AppEvent::Key(key) => match state.on_key(key, SystemCheckMode::Gate) {
                SystemCheckOutcome::None => {}
                SystemCheckOutcome::Recheck => state.recheck(),
                SystemCheckOutcome::RunFix(cmd) => {
                    state.notice = Some(run_fix(terminal, &cmd, input, state.lang));
                    state.recheck();
                }
                SystemCheckOutcome::Continue => return Ok(GateResult::Continue),
                SystemCheckOutcome::Cancel => return Ok(GateResult::Quit),
            },
            AppEvent::Shutdown => return Ok(GateResult::Quit),
            _ => {}
        }
        terminal.draw(|frame| ui::setup::render_gate(frame, frame.area(), state, &theme))?;
    }

    Ok(GateResult::Quit)
}

async fn run_app(
    terminal: &mut ratatui::DefaultTerminal,
    config: Config,
    system_check: SystemCheckState,
    tx: mpsc::Sender<AppEvent>,
    mut rx: mpsc::Receiver<AppEvent>,
    mut input: Input,
) -> Result<()> {
    let tick_rate = Duration::from_millis(config.ui.tick_rate_ms);
    let backend = Backend::detect();

    let (config_tx, config_rx) = watch::channel(config.clone());

    spawn_tick_task(tx.clone(), tick_rate);
    sysmon::spawn(tx.clone());
    lxc::poller::spawn(backend, config_rx, tx.clone());

    let mut app = App::new(config, Reload { config: config_tx }, backend, system_check);

    terminal.draw(|frame| ui::draw(frame, &app))?;

    while let Some(event) = rx.recv().await {
        app.on_event(event);

        if let Some(pending) = app.take_pending() {
            match pending {
                PendingAction::RunFix(cmd) => {
                    app.system_check.notice = Some(run_fix(terminal, &cmd, &mut input, app.lang));
                    app.system_check.recheck();
                }
                PendingAction::Console(name) => {
                    let message = match backend.cli {
                        Some(cli) => run_console(terminal, cli, &name, &mut input, app.lang),
                        None => i18n::t(app.lang, Key::MainNoRealBackend).to_string(),
                    };
                    app.toast = Some((message, Status::Info));
                }
                other => spawn_action(backend, app.config.ui.command_timeout_secs, app.lang, other, tx.clone()),
            }
        }

        if app.should_quit {
            break;
        }
        terminal.draw(|frame| ui::draw(frame, &app))?;
    }

    Ok(())
}

enum ActionOutcome {
    Done(String),
    Log { name: String, log: String },
    Info { name: String, info: String },
    RemoteCatalog(Vec<lxc::CatalogImage>),
}

fn spawn_action(
    backend: Backend,
    timeout_secs: u64,
    lang: Lang,
    action: PendingAction,
    tx: mpsc::Sender<AppEvent>,
) {
    // El catálogo remoto no comparte el toast genérico: un error acá tiene que
    // llegar al modal de descarga (que si no se queda pegado en "cargando…"),
    // no a la barra de avisos.
    let is_remote_fetch = matches!(action, PendingAction::FetchRemoteImages);
    tokio::spawn(async move {
        match execute_action(backend, timeout_secs, lang, action).await {
            Ok(ActionOutcome::Done(message)) => {
                let _ = tx.send(AppEvent::ActionResult { message, ok: true }).await;
                // Sin esto, start/stop/crear/borrar/etc. sólo se veían
                // reflejados en la tabla recién en el próximo tick del
                // poller (`poll.instances_secs`/`poll.inventory_secs`,
                // hasta varios segundos) — refresca ya mismo, el tick
                // periódico sigue andando igual como respaldo.
                tokio::join!(
                    lxc::poller::poll_instances(backend, lang, timeout_secs, &tx),
                    lxc::poller::poll_inventory(backend, lang, timeout_secs, &tx),
                );
            }
            Ok(ActionOutcome::Log { name, log }) => {
                let _ = tx.send(AppEvent::InstanceLog { name, log }).await;
            }
            Ok(ActionOutcome::Info { name, info }) => {
                let _ = tx.send(AppEvent::InstanceInfo { name, info }).await;
            }
            Ok(ActionOutcome::RemoteCatalog(catalog)) => {
                let _ = tx.send(AppEvent::RemoteImagesFetched(Ok(catalog))).await;
            }
            Err(message) => {
                if is_remote_fetch {
                    let _ = tx.send(AppEvent::RemoteImagesFetched(Err(message))).await;
                } else {
                    let _ = tx.send(AppEvent::ActionResult { message, ok: false }).await;
                }
            }
        }
    });
}

fn tpl1(lang: Lang, key: Key, a: &str) -> String {
    i18n::t(lang, key).replacen("{}", a, 1)
}

fn tpl2(lang: Lang, key: Key, a: &str, b: &str) -> String {
    i18n::t(lang, key).replacen("{}", a, 1).replacen("{}", b, 1)
}

/// Une la salida de `lxc info` y `lxc config show` bajo encabezados propios
/// para el popup de la pestaña CONTAINERS (tecla `i`) — dos comandos, un solo
/// texto scrolleable.
fn combine_info_and_config(info: String, config: String) -> String {
    format!("== lxc info ==\n{}\n== lxc config show ==\n{}", info.trim_end(), config.trim_end())
}

/// Une `lxc storage show` + `lxc storage info` + `lxc storage volume list`
/// bajo encabezados propios para el popup de la pestaña STORAGE (tecla `i`) —
/// mismo criterio que `combine_info_and_config`.
fn combine_storage_info(show: String, info: String, volumes: String) -> String {
    format!(
        "== lxc storage show ==\n{}\n== lxc storage info ==\n{}\n== lxc storage volume list ==\n{}",
        show.trim_end(),
        info.trim_end(),
        volumes
    )
}

/// Une `lxc network show` y `lxc network list-leases` bajo encabezados propios
/// para el popup de la pestaña NETWORK (tecla `i`).
fn combine_network_info(show: String, leases: String) -> String {
    format!("== lxc network show ==\n{}\n== lxc network list-leases ==\n{}", show.trim_end(), leases.trim_end())
}

/// Lista de volúmenes de un pool como texto plano para el popup combinado —
/// `- nombre (tipo, content_type)` por línea, o un aviso si el pool no tiene
/// ninguno.
/// `pool` no importa acá (todo lo que entra ya es de un único pool) — sólo
/// nombre/tipo/content_type, para no atar esta función a `VolumeEntry` ni a
/// `RawStorageVolume` en particular.
fn format_pool_volumes(lang: Lang, volumes: impl Iterator<Item = (String, String, String)>) -> String {
    let lines: Vec<String> =
        volumes.map(|(name, kind, content_type)| format!("- {name} ({kind}, {content_type})")).collect();
    if lines.is_empty() {
        i18n::t(lang, Key::StorageNoVolumesInPool).to_string()
    } else {
        lines.join("\n")
    }
}

/// Ejecuta una acción de escritura contra el backend real, o la simula en modo
/// fixtures (macOS sin LXD, o `VOXCONTAINER_FIXTURES=1`).
async fn execute_action(
    backend: Backend,
    timeout_secs: u64,
    lang: Lang,
    action: PendingAction,
) -> Result<ActionOutcome, String> {
    use lxc::{actions, fixtures, query};

    if backend.fixtures {
        return Ok(match action {
            PendingAction::FetchLog(name) => {
                let log = fixtures::instance_log(&name);
                ActionOutcome::Log { name, log }
            }
            PendingAction::FetchInfo(name) => {
                let info = fixtures::instance_info(&name);
                let config = fixtures::instance_config_show(&name);
                ActionOutcome::Info { name, info: combine_info_and_config(info, config) }
            }
            PendingAction::FetchProfileShow(name) => {
                let info = fixtures::profile_show(&name);
                ActionOutcome::Info { name, info }
            }
            PendingAction::FetchStorageInfo(pool) => {
                let show = fixtures::storage_show(&pool);
                let info = fixtures::storage_pool_info(&pool);
                let volumes = format_pool_volumes(
                    lang,
                    fixtures::volumes()
                        .into_iter()
                        .filter(|v| v.pool == pool)
                        .map(|v| (v.name, v.kind, v.content_type)),
                );
                ActionOutcome::Info { name: pool, info: combine_storage_info(show, info, volumes) }
            }
            PendingAction::FetchNetworkInfo(name) => {
                let show = fixtures::network_show(&name);
                let leases = fixtures::network_leases(&name);
                ActionOutcome::Info { name, info: combine_network_info(show, leases) }
            }
            PendingAction::FetchImageInfo(name) => {
                let info = fixtures::image_info(&name);
                ActionOutcome::Info { name, info }
            }
            PendingAction::FetchVolumeInfo { pool, volume } => {
                let info = fixtures::volume_show(&volume);
                ActionOutcome::Info { name: format!("{pool}/{volume}"), info }
            }
            PendingAction::FetchOperations => {
                ActionOutcome::Info { name: i18n::t(lang, Key::InfoTitleOperations).to_string(), info: fixtures::operation_list() }
            }
            PendingAction::FetchWarnings => {
                ActionOutcome::Info { name: i18n::t(lang, Key::InfoTitleWarnings).to_string(), info: fixtures::warning_list() }
            }
            PendingAction::FetchRemoteImages => ActionOutcome::RemoteCatalog(fixtures::remote_images()),
            other => ActionOutcome::Done(tpl1(lang, Key::MainFixturesSimulatedTemplate, describe(lang, &other))),
        });
    }

    let Some(cli) = backend.cli else {
        return Err(lxc::cli::LxcError::NotInstalled.localized(lang));
    };

    match action {
        PendingAction::Start(name) => {
            actions::start(cli, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionStartedTemplate, &name)))
        }
        PendingAction::Stop(name) => {
            actions::stop(cli, &name, false, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionStoppedTemplate, &name)))
        }
        PendingAction::Restart(name) => {
            actions::restart(cli, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionRestartedTemplate, &name)))
        }
        PendingAction::Pause(name) => {
            actions::pause(cli, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionPausedTemplate, &name)))
        }
        PendingAction::Launch { image, name, start: true, opts } => {
            actions::launch(cli, &image, &name, &opts, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionLaunchedTemplate, &name, &image)))
        }
        PendingAction::Launch { image, name, start: false, opts } => {
            actions::init_stopped(cli, &image, &name, &opts, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionCreatedStoppedTemplate, &name, &image)))
        }
        PendingAction::Copy { src, dst } => {
            actions::copy(cli, &src, &dst, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionClonedTemplate, &src, &dst)))
        }
        PendingAction::Rename { old, new } => {
            actions::rename(cli, &old, &new, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionRenamedTemplate, &old, &new)))
        }
        PendingAction::SetConfigKeys { name, pairs } => {
            actions::set_config_keys(cli, &name, &pairs, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionConfigKeysUpdatedTemplate, &name)))
        }
        PendingAction::Snapshot { name, snap } => {
            actions::snapshot(cli, &name, &snap, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionSnapshotCreatedTemplate, &name, &snap)))
        }
        PendingAction::Restore { name, snap } => {
            actions::restore(cli, &name, &snap, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionRestoredTemplate, &name, &snap)))
        }
        PendingAction::DeleteSnapshot { name, snap } => {
            actions::delete_snapshot(cli, &name, &snap, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionSnapshotDeletedTemplate, &name, &snap)))
        }
        PendingAction::DeleteInstance(name) => {
            actions::delete_instance(cli, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionDeletedTemplate, &name)))
        }
        PendingAction::SetLimits { name, cpu, memory } => {
            actions::set_limits(cli, &name, cpu.as_deref(), memory.as_deref(), timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionLimitsUpdatedTemplate, &name)))
        }
        PendingAction::SetImageAlias { fingerprint, old_alias, new_alias } => {
            if old_alias.as_deref() == Some(new_alias.as_str()) {
                return Ok(ActionOutcome::Done(i18n::t(lang, Key::ImagesAliasUnchanged).to_string()));
            }
            actions::set_image_alias(cli, &fingerprint, old_alias.as_deref(), &new_alias, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(
                lang,
                Key::ImagesAliasUpdatedTemplate,
                &fingerprint.chars().take(12).collect::<String>(),
                &new_alias,
            )))
        }
        PendingAction::CreateProfile { name, description } => {
            actions::create_profile(cli, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            if let Some(description) = description {
                actions::set_profile_description(cli, &name, &description, timeout_secs)
                    .await
                    .map_err(|e| e.localized(lang))?;
            }
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionProfileCreatedTemplate, &name)))
        }
        PendingAction::DeleteProfile(name) => {
            actions::delete_profile(cli, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionProfileDeletedTemplate, &name)))
        }
        PendingAction::SetProfileLimits { name, cpu, memory } => {
            actions::set_profile_limits(cli, &name, cpu.as_deref(), memory.as_deref(), timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionLimitsUpdatedTemplate, &name)))
        }
        PendingAction::FetchProfileShow(name) => {
            let info = query::profile_show(cli, &name, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Info { name, info })
        }
        PendingAction::DeleteImage(fingerprint) => {
            actions::delete_image(cli, &fingerprint, timeout_secs).await.map_err(|e| e.localized(lang))?;
            let label: String = fingerprint.chars().take(12).collect();
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionImageDeletedTemplate, &label)))
        }
        PendingAction::AssignProfiles { name, profiles } => {
            actions::assign_profiles(cli, &name, &profiles, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(
                lang,
                Key::MainActionProfilesAssignedTemplate,
                &name,
                &profiles.join(","),
            )))
        }
        PendingAction::SetProfileDescription { name, description } => {
            actions::set_profile_description(cli, &name, &description, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionProfileDescriptionUpdatedTemplate, &name)))
        }
        PendingAction::FetchLog(name) => {
            let log = query::instance_log(cli, &name, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Log { name, log })
        }
        PendingAction::FetchInfo(name) => {
            // Un error en cualquiera de los dos comandos no debe tirar el popup
            // entero: se muestra el mensaje de error en la sección que falló y
            // se sigue mostrando la otra, como hace `query::inventory`.
            let info = match query::instance_info(cli, &name, timeout_secs).await {
                Ok(s) => s,
                Err(e) => e.localized(lang),
            };
            let config = match query::instance_config_show(cli, &name, timeout_secs).await {
                Ok(s) => s,
                Err(e) => e.localized(lang),
            };
            Ok(ActionOutcome::Info { name, info: combine_info_and_config(info, config) })
        }
        PendingAction::FetchRemoteImages => {
            let net_timeout = timeout_secs.max(lxc::DOWNLOAD_TIMEOUT_SECS);
            let catalog = query::list_remote_catalog(cli, lxc::DOWNLOAD_REMOTES, net_timeout)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::RemoteCatalog(catalog))
        }
        PendingAction::DownloadImage { reference, alias } => {
            let net_timeout = timeout_secs.max(lxc::DOWNLOAD_TIMEOUT_SECS);
            actions::download_image(cli, &reference, alias.as_deref(), net_timeout)
                .await
                .map_err(|e| e.localized(lang))?;
            let label = alias.unwrap_or(reference);
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionImageDownloadedTemplate, &label)))
        }
        PendingAction::CreateVolume { pool, name, size } => {
            actions::create_volume(cli, &pool, &name, size.as_deref(), timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionVolumeCreatedTemplate, &name, &pool)))
        }
        PendingAction::DeleteVolume { pool, name } => {
            actions::delete_volume(cli, &pool, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionVolumeDeletedTemplate, &name, &pool)))
        }
        PendingAction::AttachVolume { pool, name, instance, path } => {
            actions::attach_volume(cli, &pool, &name, &instance, &path, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionVolumeAttachedTemplate, &name, &instance)))
        }
        PendingAction::DetachVolume { pool, name, instance } => {
            actions::detach_volume(cli, &pool, &name, &instance, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionVolumeDetachedTemplate, &name, &instance)))
        }
        PendingAction::CreatePool { name, driver, source } => {
            actions::create_pool(cli, &name, &driver, source.as_deref(), timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionPoolCreatedTemplate, &name, &driver)))
        }
        PendingAction::DeletePool(name) => {
            actions::delete_pool(cli, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionPoolDeletedTemplate, &name)))
        }
        PendingAction::FetchStorageInfo(pool) => {
            let show = match query::storage_show(cli, &pool, timeout_secs).await {
                Ok(s) => s,
                Err(e) => e.localized(lang),
            };
            let info = match query::storage_pool_info(cli, &pool, timeout_secs).await {
                Ok(s) => s,
                Err(e) => e.localized(lang),
            };
            let volumes = match query::list_storage_volumes(cli, &pool, timeout_secs).await {
                Ok(vols) => {
                    format_pool_volumes(lang, vols.into_iter().map(|v| (v.name, v.kind, v.content_type)))
                }
                Err(e) => e.localized(lang),
            };
            Ok(ActionOutcome::Info { name: pool, info: combine_storage_info(show, info, volumes) })
        }
        PendingAction::SetVolumeConfigKeys { pool, name, pairs } => {
            actions::set_volume_config_keys(cli, &pool, &name, &pairs, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionVolumeConfigUpdatedTemplate, &name)))
        }
        PendingAction::RenameVolume { pool, old, new } => {
            actions::rename_volume(cli, &pool, &old, &new, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionVolumeRenamedTemplate, &old, &new)))
        }
        PendingAction::ExportInstance { name, path } => {
            let net_timeout = timeout_secs.max(lxc::DOWNLOAD_TIMEOUT_SECS);
            actions::export_instance(cli, &name, &path, net_timeout).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionInstanceExportedTemplate, &name, &path)))
        }
        PendingAction::ImportBackup { path } => {
            let net_timeout = timeout_secs.max(lxc::DOWNLOAD_TIMEOUT_SECS);
            actions::import_backup(cli, &path, net_timeout).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionBackupImportedTemplate, &path)))
        }
        PendingAction::CreateNetwork { name, pairs } => {
            actions::create_network(cli, &name, &pairs, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionNetworkCreatedTemplate, &name)))
        }
        PendingAction::AttachNetwork { network, instance, device } => {
            actions::attach_network(cli, &network, &instance, device.as_deref(), timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionNetworkAttachedTemplate, &network, &instance)))
        }
        PendingAction::UnsetConfigKeys { name, keys } => {
            actions::unset_config_keys(cli, &name, &keys, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionConfigKeysUnsetTemplate, &name, &keys.join(","))))
        }
        PendingAction::TransferFile { name, push, local, remote, recursive } => {
            let net_timeout = timeout_secs.max(lxc::DOWNLOAD_TIMEOUT_SECS);
            actions::transfer_file(cli, push, &name, &local, &remote, recursive, net_timeout)
                .await
                .map_err(|e| e.localized(lang))?;
            let key = if push { Key::MainActionFilePushedTemplate } else { Key::MainActionFilePulledTemplate };
            Ok(ActionOutcome::Done(tpl2(lang, key, &local, &name)))
        }
        PendingAction::AddDevice { name, device, kind, pairs } => {
            actions::add_device(cli, &name, &device, &kind, &pairs, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionDeviceAddedTemplate, &device, &name)))
        }
        PendingAction::RemoveDevice { name, device } => {
            actions::remove_device(cli, &name, &device, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionDeviceRemovedTemplate, &device, &name)))
        }
        PendingAction::DeleteNetwork(name) => {
            actions::delete_network(cli, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionNetworkDeletedTemplate, &name)))
        }
        PendingAction::SetNetworkKeys { name, pairs } => {
            actions::set_network_keys(cli, &name, &pairs, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionNetworkConfigUpdatedTemplate, &name)))
        }
        PendingAction::FetchNetworkInfo(name) => {
            let show = match query::network_show(cli, &name, timeout_secs).await {
                Ok(s) => s,
                Err(e) => e.localized(lang),
            };
            let leases = match query::network_leases(cli, &name, timeout_secs).await {
                Ok(s) => s,
                Err(e) => e.localized(lang),
            };
            Ok(ActionOutcome::Info { name, info: combine_network_info(show, leases) })
        }
        PendingAction::PublishInstance { name, alias } => {
            let net_timeout = timeout_secs.max(lxc::DOWNLOAD_TIMEOUT_SECS);
            actions::publish_instance(cli, &name, alias.as_deref(), net_timeout)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionInstancePublishedTemplate, &name)))
        }
        PendingAction::FetchImageInfo(name) => {
            let info = query::image_info(cli, &name, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Info { name, info })
        }
        PendingAction::ExportImage { image, dir } => {
            let net_timeout = timeout_secs.max(lxc::DOWNLOAD_TIMEOUT_SECS);
            actions::export_image(cli, &image, &dir, net_timeout).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionImageExportedTemplate, &image, &dir)))
        }
        PendingAction::ImportImage { path, alias } => {
            let net_timeout = timeout_secs.max(lxc::DOWNLOAD_TIMEOUT_SECS);
            actions::import_image(cli, &path, alias.as_deref(), net_timeout)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl1(lang, Key::MainActionImageImportedTemplate, &path)))
        }
        PendingAction::FetchVolumeInfo { pool, volume } => {
            let info = query::volume_show(cli, &pool, &volume, timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Info { name: format!("{pool}/{volume}"), info })
        }
        PendingAction::SnapshotVolume { pool, volume, snap } => {
            actions::snapshot_volume(cli, &pool, &volume, snap.as_deref(), timeout_secs)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionVolumeSnapshotTemplate, &volume, &pool)))
        }
        PendingAction::MoveVolume { pool, volume, dest_pool } => {
            let net_timeout = timeout_secs.max(lxc::DOWNLOAD_TIMEOUT_SECS);
            actions::move_volume(cli, &pool, &volume, &dest_pool, net_timeout)
                .await
                .map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Done(tpl2(lang, Key::MainActionVolumeMovedTemplate, &volume, &dest_pool)))
        }
        PendingAction::FetchOperations => {
            let info = query::operation_list(cli, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Info { name: i18n::t(lang, Key::InfoTitleOperations).to_string(), info })
        }
        PendingAction::FetchWarnings => {
            let info = query::warning_list(cli, timeout_secs).await.map_err(|e| e.localized(lang))?;
            Ok(ActionOutcome::Info { name: i18n::t(lang, Key::InfoTitleWarnings).to_string(), info })
        }
        PendingAction::RunFix(_) | PendingAction::Console(_) => {
            Err(i18n::t(lang, Key::MainDescribeAction).to_string())
        }
    }
}

fn describe(lang: Lang, action: &PendingAction) -> &'static str {
    match action {
        PendingAction::Start(_) => i18n::t(lang, Key::MainDescribeStart),
        PendingAction::Stop(_) => i18n::t(lang, Key::MainDescribeStop),
        PendingAction::Restart(_) => i18n::t(lang, Key::MainDescribeRestart),
        PendingAction::Pause(_) => i18n::t(lang, Key::MainDescribePause),
        PendingAction::Launch { .. } => i18n::t(lang, Key::MainDescribeLaunch),
        PendingAction::Copy { .. } => i18n::t(lang, Key::MainDescribeCopy),
        PendingAction::Rename { .. } => i18n::t(lang, Key::MainDescribeRename),
        PendingAction::SetConfigKeys { .. } => i18n::t(lang, Key::MainDescribeConfigKeys),
        PendingAction::Snapshot { .. } => "snapshot",
        PendingAction::Restore { .. } => i18n::t(lang, Key::MainDescribeRestore),
        PendingAction::DeleteSnapshot { .. } => i18n::t(lang, Key::MainDescribeDeleteSnapshot),
        PendingAction::DeleteInstance(_) => i18n::t(lang, Key::MainDescribeDelete),
        PendingAction::SetLimits { .. } => i18n::t(lang, Key::MainDescribeLimits),
        PendingAction::SetProfileLimits { .. } => i18n::t(lang, Key::MainDescribeLimits),
        PendingAction::SetImageAlias { .. } => i18n::t(lang, Key::MainDescribeAlias),
        PendingAction::CreateProfile { .. } => i18n::t(lang, Key::MainDescribeCreateProfile),
        PendingAction::DeleteProfile(_) => i18n::t(lang, Key::MainDescribeDeleteProfile),
        PendingAction::DeleteImage(_) => i18n::t(lang, Key::MainDescribeDeleteImage),
        PendingAction::AssignProfiles { .. } => i18n::t(lang, Key::MainDescribeAssignProfiles),
        PendingAction::SetProfileDescription { .. } => i18n::t(lang, Key::MainDescribeEditProfileDescription),
        PendingAction::DownloadImage { .. } => i18n::t(lang, Key::MainDescribeDownloadImage),
        PendingAction::FetchRemoteImages => i18n::t(lang, Key::MainDescribeFetchCatalog),
        PendingAction::CreateVolume { .. } => i18n::t(lang, Key::MainDescribeCreateVolume),
        PendingAction::DeleteVolume { .. } => i18n::t(lang, Key::MainDescribeDeleteVolume),
        PendingAction::AttachVolume { .. } => i18n::t(lang, Key::MainDescribeAttachVolume),
        PendingAction::DetachVolume { .. } => i18n::t(lang, Key::MainDescribeDetachVolume),
        PendingAction::CreatePool { .. } => i18n::t(lang, Key::MainDescribeCreatePool),
        PendingAction::DeletePool(_) => i18n::t(lang, Key::MainDescribeDeletePool),
        PendingAction::SetVolumeConfigKeys { .. } => i18n::t(lang, Key::MainDescribeConfigKeys),
        PendingAction::RenameVolume { .. } => i18n::t(lang, Key::MainDescribeRename),
        PendingAction::ExportInstance { .. } => i18n::t(lang, Key::MainDescribeExport),
        PendingAction::ImportBackup { .. } => i18n::t(lang, Key::MainDescribeImport),
        PendingAction::CreateNetwork { .. } => i18n::t(lang, Key::MainDescribeCreateNetwork),
        PendingAction::AttachNetwork { .. } => i18n::t(lang, Key::MainDescribeAttachNetwork),
        PendingAction::UnsetConfigKeys { .. } => i18n::t(lang, Key::MainDescribeUnset),
        PendingAction::TransferFile { .. } => i18n::t(lang, Key::MainDescribeTransferFile),
        PendingAction::AddDevice { .. } => i18n::t(lang, Key::MainDescribeAddDevice),
        PendingAction::RemoveDevice { .. } => i18n::t(lang, Key::MainDescribeRemoveDevice),
        PendingAction::DeleteNetwork(_) => i18n::t(lang, Key::MainDescribeDeleteNetwork),
        PendingAction::SetNetworkKeys { .. } => i18n::t(lang, Key::MainDescribeConfigKeys),
        PendingAction::PublishInstance { .. } => i18n::t(lang, Key::MainDescribePublish),
        PendingAction::ExportImage { .. } => i18n::t(lang, Key::MainDescribeExportImage),
        PendingAction::ImportImage { .. } => i18n::t(lang, Key::MainDescribeImportImage),
        PendingAction::SnapshotVolume { .. } => i18n::t(lang, Key::MainDescribeSnapshotVolume),
        PendingAction::MoveVolume { .. } => i18n::t(lang, Key::MainDescribeMoveVolume),
        PendingAction::FetchStorageInfo(_)
        | PendingAction::FetchNetworkInfo(_)
        | PendingAction::FetchImageInfo(_)
        | PendingAction::FetchVolumeInfo { .. }
        | PendingAction::FetchOperations
        | PendingAction::FetchWarnings => i18n::t(lang, Key::MainDescribeAction),
        PendingAction::FetchLog(_)
        | PendingAction::FetchInfo(_)
        | PendingAction::FetchProfileShow(_)
        | PendingAction::RunFix(_)
        | PendingAction::Console(_) => i18n::t(lang, Key::MainDescribeAction),
    }
}

/// Sale de la pantalla alterna, corre el comando de remediación heredando la
/// terminal (para que `sudo` pueda pedir la contraseña), y vuelve a entrar.
fn run_fix(terminal: &mut ratatui::DefaultTerminal, command: &[String], input: &mut Input, lang: Lang) -> String {
    input.stop();
    ratatui::restore();

    println!("\n>> {}\n", deps::command_line(command));
    let outcome = deps::run_fix(command);

    println!("\n{}", i18n::t(lang, Key::MainPressEnterToReturn));
    let mut discard = String::new();
    let _ = std::io::stdin().read_line(&mut discard);

    *terminal = ratatui::init();
    input.start();

    match outcome {
        Ok(true) => i18n::t(lang, Key::MainFixDone).to_string(),
        Ok(false) => i18n::t(lang, Key::MainFixError).to_string(),
        Err(e) => tpl1(lang, Key::MainFixExecErrTemplate, &e.to_string()),
    }
}

/// Igual que `run_fix` pero para `lxc exec`: abre una shell dentro del
/// contenedor, heredando la terminal real — `lxc exec` necesita el tty.
fn run_console(
    terminal: &mut ratatui::DefaultTerminal,
    cli: Cli,
    name: &str,
    input: &mut Input,
    lang: Lang,
) -> String {
    input.stop();
    ratatui::restore();

    println!("\n>> {} exec {name} -- sh -c 'exec bash || exec sh'\n", cli.binary());
    let status = std::process::Command::new(cli.binary())
        .args(["exec", name, "--", "sh", "-c", "exec bash || exec sh"])
        .status();

    println!("\n{}", i18n::t(lang, Key::MainPressEnterToReturn));
    let mut discard = String::new();
    let _ = std::io::stdin().read_line(&mut discard);

    *terminal = ratatui::init();
    input.start();

    match status {
        Ok(s) if s.success() => i18n::t(lang, Key::MainConsoleClosed).to_string(),
        Ok(_) => i18n::t(lang, Key::MainConsoleError).to_string(),
        Err(e) => tpl1(lang, Key::MainConsoleOpenErrTemplate, &e.to_string()),
    }
}

fn spawn_input_task(tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut stream = EventStream::new();
        while let Some(Ok(evt)) = stream.next().await {
            if let Event::Key(key) = evt
                && tx.send(AppEvent::Key(key)).await.is_err()
            {
                break;
            }
        }
    })
}

fn spawn_tick_task(tx: mpsc::Sender<AppEvent>, tick_rate: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick_rate);
        loop {
            interval.tick().await;
            if tx.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });
}

/// SIGTERM (systemd stop, `kill`, cierre de sesión) no llega como KeyEvent de
/// crossterm: sin este handler el proceso muere antes de restaurar la terminal.
fn spawn_signal_task(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let mut sigterm =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(s) => s,
                Err(_) => return,
            };
        sigterm.recv().await;
        let _ = tx.send(AppEvent::Shutdown).await;
    });
}
