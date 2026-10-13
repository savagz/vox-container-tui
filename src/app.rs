use std::collections::HashMap;
use std::time::Instant;

use crossterm::event::{KeyEvent, KeyEventKind};
use tokio::sync::watch;

use crate::config::{Config, ConfigForm, ConfigOutcome};
use crate::event::AppEvent;
use crate::i18n::{self, Key, Lang};
use crate::keys::{self, AppAction};
use crate::lxc::{
    Backend, CatalogImage, Instance, Inventory, InstanceStatus, LaunchOptions, StoragePool, VolumeEntry,
    STORAGE_DRIVERS,
};
use crate::setup::{SystemCheckMode, SystemCheckOutcome, SystemCheckState};
use crate::sysmon::snapshot::HostSnapshot;
use crate::theme::{Status, Theme};
use crate::ui::tabs::Tab;

/// Canal de recarga en caliente hacia el poller. Guardar en el overlay de
/// configuración empuja la config nueva por acá: los intervalos de refresco y el
/// timeout de comandos se aplican solos, sin reiniciar el proceso.
pub struct Reload {
    pub config: watch::Sender<Config>,
}

/// Acciones de escritura sobre instancias/dependencias. Se resuelven fuera de
/// `App` porque necesitan async (correr `lxc`/`incus`) o adueñarse de la terminal
/// (correr `sudo`, abrir una consola con `lxc exec`).
#[derive(Debug, Clone)]
pub enum PendingAction {
    Start(String),
    /// `lxc stop <name>` sin `--force`: la instancia debe estar corriendo.
    Stop(String),
    Restart(String),
    Pause(String),
    /// `start: false` crea la instancia detenida (`lxc init`) — para setear
    /// límites antes del primer boot.
    Launch { image: String, name: String, start: bool, opts: LaunchOptions },
    Copy { src: String, dst: String },
    /// `lxc move <old> <new>` — renombra la instancia.
    Rename { old: String, new: String },
    Snapshot { name: String, snap: String },
    Restore { name: String, snap: String },
    DeleteSnapshot { name: String, snap: String },
    DeleteInstance(String),
    SetLimits {
        name: String,
        cpu: Option<String>,
        memory: Option<String>,
    },
    /// `lxc config set <name> <key>=<value>` genérico, uno o más pares — a
    /// diferencia de `SetLimits`, no está atado a `limits.cpu`/`limits.memory`.
    SetConfigKeys {
        name: String,
        pairs: Vec<(String, String)>,
    },
    /// `lxc profile set <name> limits.cpu=<cpu>` / `limits.memory=<memory>` —
    /// igual que `SetLimits` pero sobre un perfil, no una instancia.
    SetProfileLimits {
        name: String,
        cpu: Option<String>,
        memory: Option<String>,
    },
    FetchLog(String),
    /// `lxc info <name>` para el popup de la pestaña CONTAINERS.
    FetchInfo(String),
    /// `lxc profile show <name>` para el popup de la pestaña PROFILES —
    /// comparte el mismo popup de sólo lectura que `FetchInfo` (misma forma
    /// de resultado: un nombre + un texto).
    FetchProfileShow(String),
    /// Requiere suspender la TUI para que `sudo` pueda pedir la contraseña.
    RunFix(Vec<String>),
    /// Requiere suspender la TUI: `lxc exec` toma el tty.
    Console(String),
    /// Redefine el alias de una imagen: borra `old_alias` (si tenía uno
    /// distinto) y crea `new_alias` apuntando al mismo fingerprint.
    SetImageAlias {
        fingerprint: String,
        old_alias: Option<String>,
        new_alias: String,
    },
    /// Borra una imagen local por fingerprint (pestaña IMAGES, tabla local —
    /// no confundir con el catálogo remoto del modal de descarga).
    DeleteImage(String),
    /// `description` (si vino) se aplica con un segundo llamado (`profile
    /// edit`) después de crear — `profile create` no tiene flag de descripción.
    CreateProfile { name: String, description: Option<String> },
    DeleteProfile(String),
    AssignProfiles { name: String, profiles: Vec<String> },
    SetProfileDescription { name: String, description: String },
    /// Lista el catálogo combinado de `lxc::DOWNLOAD_REMOTES` para el modal de
    /// descarga (pestaña IMAGES). No toca el daemon local — son llamadas de red.
    FetchRemoteImages,
    /// `lxc image copy <reference> local: [--alias <alias>]` — trae una imagen
    /// del catálogo remoto al almacén local.
    DownloadImage { reference: String, alias: Option<String> },
    /// `lxc storage volume create <pool> <name> [size=<size>]`.
    CreateVolume { pool: String, name: String, size: Option<String> },
    /// `lxc storage volume delete <pool> <name>`.
    DeleteVolume { pool: String, name: String },
    /// `lxc storage volume attach <pool> <name> <instance>`.
    AttachVolume { pool: String, name: String, instance: String, path: String },
    /// `lxc storage volume detach <pool> <name> <instance>`.
    DetachVolume { pool: String, name: String, instance: String },
    /// `lxc storage create <name> <driver> [source=<ruta>]`.
    CreatePool { name: String, driver: String, source: Option<String> },
    /// `lxc storage delete <name>`.
    DeletePool(String),
    /// `lxc storage show <pool>` + `lxc storage info <pool>` + `lxc storage
    /// volume list <pool>` combinados para el popup de la pestaña STORAGE.
    FetchStorageInfo(String),
    /// `lxc storage volume set <pool> <name> <key>=<value>` genérico — igual
    /// que `SetConfigKeys` pero sobre un volumen.
    SetVolumeConfigKeys { pool: String, name: String, pairs: Vec<(String, String)> },
    /// `lxc storage volume rename <pool> <old> <new>`.
    RenameVolume { pool: String, old: String, new: String },
    /// `lxc export <name> <ruta>` — backup completo de la instancia a un
    /// archivo tarball.
    ExportInstance { name: String, path: String },
    /// `lxc import <ruta>` — crea una instancia nueva a partir de un backup.
    ImportBackup { path: String },
    /// `lxc network create <name> [<key>=<value>...]`.
    CreateNetwork { name: String, pairs: Vec<(String, String)> },
    /// `lxc network attach <network> <instance> [<device>]` — sin nombre de
    /// device explícito, LXD/Incus usan el nombre de la red.
    AttachNetwork { network: String, instance: String, device: Option<String> },
    /// `lxc config unset <name> <clave>` por cada clave.
    UnsetConfigKeys { name: String, keys: Vec<String> },
    /// `lxc file push|pull` entre el host y la instancia.
    TransferFile { name: String, push: bool, local: String, remote: String, recursive: bool },
    /// `lxc config device add <name> <device> <kind> [clave=valor...]`.
    AddDevice { name: String, device: String, kind: String, pairs: Vec<(String, String)> },
    /// `lxc config device remove <name> <device>`.
    RemoveDevice { name: String, device: String },
    /// `lxc network delete <name>`.
    DeleteNetwork(String),
    /// `lxc network set <name> <clave>=<valor>...`.
    SetNetworkKeys { name: String, pairs: Vec<(String, String)> },
    /// `lxc network show` + `lxc network list-leases` para el popup de NETWORK.
    FetchNetworkInfo(String),
    /// `lxc publish <name> [--alias <alias>]`.
    PublishInstance { name: String, alias: Option<String> },
    /// `lxc image info <fingerprint>` para el popup de IMAGES.
    FetchImageInfo(String),
    /// `lxc image export <image> <dir>`.
    ExportImage { image: String, dir: String },
    /// `lxc image import <path> [--alias <alias>]`.
    ImportImage { path: String, alias: Option<String> },
    /// `lxc storage volume show <pool> <volume>` para el popup de STORAGE.
    FetchVolumeInfo { pool: String, volume: String },
    /// `lxc storage volume snapshot <pool> <volume> [<snap>]`.
    SnapshotVolume { pool: String, volume: String, snap: Option<String> },
    /// `lxc storage volume move <pool>/<volume> <dest_pool>/<volume>`.
    MoveVolume { pool: String, volume: String, dest_pool: String },
    /// `lxc operation list` para el popup de LOGS.
    FetchOperations,
    /// `lxc warning list` para el popup de LOGS.
    FetchWarnings,
}

#[derive(Debug, Clone)]
pub enum ConfirmAction {
    DeleteInstance(String),
    RestoreSnapshot { name: String, snap: String },
    DeleteSnapshot { name: String, snap: String },
    DeleteProfile(String),
    /// Fingerprint completo — lo que el usuario tipea para confirmar es el
    /// alias o el fingerprint corto, guardado aparte en `ConfirmState.expected`.
    DeleteImage(String),
    DeleteVolume { pool: String, name: String },
    DeletePool(String),
    DeleteNetwork(String),
}

/// Modal de confirmación destructiva: exige tipear el nombre exacto de la
/// instancia, no un `y/n` — un `Supr` accidental sobre una lista ordenable no
/// debe poder borrar el contenedor equivocado.
#[derive(Debug, Clone)]
pub struct ConfirmState {
    pub action: ConfirmAction,
    pub expected: String,
    pub input: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormKind {
    NewInstance,
    Clone,
    Rename,
    Snapshot,
    Limits,
    ConfigKeys,
    Alias,
    NewProfile,
    AssignProfiles,
    EditProfileDescription,
    ProfileLimits,
    NewVolume,
    AttachVolume,
    DetachVolume,
    NewPool,
    VolumeConfigKeys,
    RenameVolume,
    ExportInstance,
    ImportBackup,
    NewNetwork,
    AttachNetwork,
    UnsetConfigKeys,
    TransferFile,
    Device,
    NetworkConfigKeys,
    PublishInstance,
    ExportImage,
    ImportImage,
    SnapshotVolume,
    MoveVolume,
}

/// Formulario genérico de 1-3 campos para crear/clonar/snapshotear/limitar. El
/// mismo widget (`ui::widgets::form`) sirve a todos los casos. Un campo es de
/// texto libre por default; si su entrada en `select_options` trae `Some(lista)`
/// pasa a ser cíclico (`◀ ▶`, ver `App::on_form_key`) en vez de tipeable —
/// `Vec<String>` (no `&'static str`) porque algunas listas son datos en vivo
/// (instancias existentes), no constantes (drivers de storage).
#[derive(Debug, Clone)]
pub struct ActionForm {
    pub kind: FormKind,
    pub target: String,
    pub labels: Vec<&'static str>,
    pub values: Vec<String>,
    pub select_options: Vec<Option<Vec<String>>>,
    pub focus: usize,
    pub error: Option<String>,
}

impl ActionForm {
    fn new(kind: FormKind, target: String, labels: Vec<&'static str>, values: Vec<String>) -> Self {
        let field_count = labels.len();
        Self {
            kind,
            target,
            labels,
            values,
            select_options: vec![None; field_count],
            focus: 0,
            error: None,
        }
    }

    /// Marca el campo `index` como selector cíclico sobre `options` en vez de
    /// texto libre. No-op si `options` viene vacía (ej. sin instancias
    /// todavía) — mejor dejarlo como texto libre que un selector sin nada
    /// para elegir.
    fn with_select(mut self, index: usize, options: Vec<String>) -> Self {
        if !options.is_empty() {
            self.select_options[index] = Some(options);
        }
        self
    }
}

/// Estado del modal de descarga de imágenes (pestaña IMAGES, tecla `W`):
/// catálogo combinado de `lxc::DOWNLOAD_REMOTES`, fila seleccionada y filtro de
/// búsqueda propio (matchea también por remote) — separado del filtro de
/// CONTAINERS porque son listas distintas y pueden coexistir en el tiempo
/// (aunque no en pantalla, al ser modales exclusivos).
#[derive(Debug, Clone, Default)]
pub struct ImageCatalogState {
    pub loading: bool,
    pub error: Option<String>,
    pub catalog: Vec<CatalogImage>,
    pub row: usize,
    pub filter: String,
    pub filter_active: bool,
}

impl ImageCatalogState {
    pub fn filtered(&self) -> Vec<&CatalogImage> {
        if self.filter.is_empty() {
            return self.catalog.iter().collect();
        }
        let needle = self.filter.to_lowercase();
        self.catalog
            .iter()
            .filter(|entry| {
                let img = &entry.image;
                entry.remote.to_lowercase().contains(&needle)
                    || img.aliases.iter().any(|a| a.name.to_lowercase().contains(&needle))
                    || img.description().map(|d| d.to_lowercase().contains(&needle)).unwrap_or(false)
                    || img.architecture.to_lowercase().contains(&needle)
                    || img.fingerprint.to_lowercase().contains(&needle)
            })
            .collect()
    }
}

pub struct App {
    pub should_quit: bool,
    pub active_tab: Tab,
    pub theme: Theme,
    pub lang: Lang,
    pub config: Config,
    pub reload: Reload,
    pub backend: Backend,

    pub config_form: Option<ConfigForm>,
    pub system_check: SystemCheckState,
    pub confirm: Option<ConfirmState>,
    pub form: Option<ActionForm>,
    pub image_catalog: Option<ImageCatalogState>,
    pub pending: Option<PendingAction>,
    pub toast: Option<(String, Status)>,
    pub help_open: bool,
    /// Línea desde la que se dibuja el modal de ayuda — igual que
    /// `info_scroll`, se resetea cada vez que se abre.
    pub help_scroll: u16,

    pub instances: Vec<Instance>,
    prev_cpu: HashMap<String, (u64, Instant)>,
    pub inventory: Inventory,
    pub lxc_error: Option<String>,
    pub host: HostSnapshot,

    pub containers_row: usize,
    pub snapshot_row: usize,
    pub image_row: usize,
    pub profile_row: usize,
    pub network_row: usize,
    /// Fila seleccionada en la tabla de POOLS (nivel de entrada de STORAGE).
    pub storage_row: usize,
    /// Fila seleccionada entre los volúmenes del pool activo — sólo tiene
    /// sentido con `storage_drill` en `Some`.
    pub storage_volume_row: usize,
    /// `None` = STORAGE muestra la tabla de pools; `Some(pool)` = muestra los
    /// volúmenes de ese pool (drill-down con `Enter`, `Esc` vuelve a `None`).
    pub storage_drill: Option<String>,
    pub selected_instance_name: Option<String>,

    pub filter_active: bool,
    pub filter_input: String,

    pub log_text: Option<String>,
    pub log_loading: bool,

    /// Popup de `lxc info` (pestaña CONTAINERS, tecla `i`): abierto/cargando +
    /// texto, y el nombre de la instancia pedida — evita que una respuesta
    /// tardía pise el popup si mientras tanto se cerró o cambió de instancia.
    pub info_open: bool,
    pub info_loading: bool,
    pub info_text: Option<String>,
    pub info_target: Option<String>,
    /// Qué pedir al apretar `R` dentro del popup: cada popup (instancia,
    /// perfil, pool, red...) re-pide su propio comando.
    pub info_refresh: Option<PendingAction>,
    /// Línea desde la que se dibuja el texto — el popup no entra entero en
    /// pantalla (`lxc info` + `lxc config show` concatenados puede superar el
    /// alto disponible fácil).
    pub info_scroll: u16,
}

impl App {
    pub fn new(config: Config, reload: Reload, backend: Backend, system_check: SystemCheckState) -> Self {
        Self {
            should_quit: false,
            active_tab: Tab::default(),
            theme: Theme::default(),
            lang: config.ui.language,
            config,
            reload,
            backend,
            config_form: None,
            system_check,
            confirm: None,
            form: None,
            image_catalog: None,
            pending: None,
            toast: None,
            help_open: false,
            help_scroll: 0,
            instances: Vec::new(),
            prev_cpu: HashMap::new(),
            inventory: Inventory::default(),
            lxc_error: None,
            host: HostSnapshot::default(),
            containers_row: 0,
            snapshot_row: 0,
            image_row: 0,
            profile_row: 0,
            network_row: 0,
            storage_row: 0,
            storage_volume_row: 0,
            storage_drill: None,
            selected_instance_name: None,
            filter_active: false,
            filter_input: String::new(),
            log_text: None,
            log_loading: false,
            info_open: false,
            info_loading: false,
            info_text: None,
            info_target: None,
            info_refresh: None,
            info_scroll: 0,
        }
    }

    pub fn take_pending(&mut self) -> Option<PendingAction> {
        self.pending.take()
    }

    pub fn on_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.on_key(key),
            AppEvent::Tick => {}
            AppEvent::InstancesUpdated(instances) => self.on_instances_updated(instances),
            AppEvent::InventoryUpdated(inventory) => {
                self.inventory = inventory;
                self.lxc_error = None;
            }
            AppEvent::InstanceLog { name, log } => {
                self.log_loading = false;
                // El usuario pudo haber cambiado de instancia mientras el pedido
                // estaba en vuelo; un log obsoleto no debe pisar la vista actual.
                if self.selected_instance_name.as_deref() == Some(name.as_str()) {
                    self.log_text = Some(log);
                }
            }
            AppEvent::InstanceInfo { name, info } => {
                self.info_loading = false;
                // Igual que con el log: si mientras tanto se cerró el popup o
                // se pidió info de otra instancia, una respuesta obsoleta no
                // debe pisar lo que hay en pantalla.
                if self.info_open && self.info_target.as_deref() == Some(name.as_str()) {
                    self.info_text = Some(info);
                }
            }
            AppEvent::RemoteImagesFetched(result) => {
                if let Some(state) = self.image_catalog.as_mut() {
                    state.loading = false;
                    match result {
                        Ok(catalog) => {
                            state.catalog = catalog;
                            state.row = 0;
                            state.error = None;
                        }
                        Err(e) => state.error = Some(e),
                    }
                }
            }
            AppEvent::LxcError(err) => self.lxc_error = Some(err),
            AppEvent::ActionResult { message, ok } => {
                let status = if ok { Status::Ok } else { Status::Error };
                self.toast = Some((message, status));
            }
            AppEvent::SysSnapshot(snapshot) => self.host = snapshot,
            AppEvent::Shutdown => self.should_quit = true,
        }
    }

    fn on_instances_updated(&mut self, instances: Vec<Instance>) {
        let now = Instant::now();
        let names: std::collections::HashSet<_> = instances.iter().map(|i| i.name.clone()).collect();
        self.prev_cpu.retain(|k, _| names.contains(k));

        let mut instances = instances;
        for inst in instances.iter_mut() {
            if let Some((prev_ns, prev_t)) = self.prev_cpu.get(&inst.name).copied() {
                let dt = now.duration_since(prev_t).as_secs_f64();
                if dt > 0.0 && inst.cpu_ns >= prev_ns {
                    let delta_ns = (inst.cpu_ns - prev_ns) as f64;
                    inst.cpu_pct = Some(delta_ns / (dt * 1_000_000_000.0) * 100.0);
                }
            }
            self.prev_cpu.insert(inst.name.clone(), (inst.cpu_ns, now));
        }

        if self.selected_instance_name.is_none() {
            self.selected_instance_name = instances.first().map(|i| i.name.clone());
        }

        self.instances = instances;
        self.lxc_error = None;
    }

    pub fn filtered_instances(&self) -> Vec<&Instance> {
        if self.filter_input.is_empty() {
            return self.instances.iter().collect();
        }
        let needle = self.filter_input.to_lowercase();
        self.instances
            .iter()
            .filter(|i| {
                i.name.to_lowercase().contains(&needle)
                    || i.status.label().to_lowercase().contains(&needle)
                    || i.status.localized(self.lang).to_lowercase().contains(&needle)
            })
            .collect()
    }

    pub fn selected_instance(&self) -> Option<&Instance> {
        self.instances
            .iter()
            .find(|i| Some(&i.name) == self.selected_instance_name.as_ref())
    }

    fn on_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if let Some(mut confirm) = self.confirm.take() {
            if self.on_confirm_key(&mut confirm, key) {
                self.confirm = Some(confirm);
            }
            return;
        }

        if self.form.is_some() {
            self.on_form_key(key);
            return;
        }

        if self.config_form.is_some() {
            self.on_config_key(key);
            return;
        }

        if self.help_open {
            self.on_help_key(key);
            return;
        }

        if self.info_open {
            self.on_info_key(key);
            return;
        }

        if let Some(mut state) = self.image_catalog.take() {
            if self.on_image_catalog_key(&mut state, key) {
                self.image_catalog = Some(state);
            }
            return;
        }

        if self.filter_active {
            self.on_filter_key(key);
            return;
        }

        self.toast = None;

        if self.active_tab == Tab::SystemCheck {
            match self.system_check.on_key(key, SystemCheckMode::Panel) {
                SystemCheckOutcome::None => {}
                SystemCheckOutcome::Recheck => self.system_check.recheck(),
                SystemCheckOutcome::RunFix(cmd) => self.pending = Some(PendingAction::RunFix(cmd)),
                SystemCheckOutcome::Continue | SystemCheckOutcome::Cancel => {}
            }
        }

        match keys::action_for(key) {
            Some(AppAction::Quit) => self.should_quit = true,
            Some(AppAction::OpenConfig) => self.config_form = Some(ConfigForm::new(&self.config)),
            Some(AppAction::Help) => {
                self.help_open = true;
                self.help_scroll = 0;
            }
            Some(AppAction::Refresh) => match self.active_tab {
                Tab::SystemCheck => {
                    self.system_check.recheck();
                    self.toast = Some((i18n::t(self.lang, Key::ToastChecksRerun).to_string(), Status::Info));
                }
                Tab::Logs => self.fetch_log(),
                _ => {
                    self.toast = Some((i18n::t(self.lang, Key::ToastAutoRefresh).to_string(), Status::Info));
                }
            },
            Some(AppAction::NextTab) => {
                self.active_tab = self.active_tab.next();
            }
            Some(AppAction::PrevTab) => {
                self.active_tab = self.active_tab.prev();
            }
            Some(AppAction::JumpTab(i)) => {
                if let Some(tab) = Tab::from_index(i) {
                    self.active_tab = tab;
                }
            }
            Some(AppAction::Filter) if self.active_tab == Tab::Containers => {
                self.filter_active = true;
            }
            Some(AppAction::Up) => self.move_selection(-1),
            Some(AppAction::Down) => self.move_selection(1),
            Some(AppAction::Top) => self.move_selection(i32::MIN),
            Some(AppAction::Bottom) => self.move_selection(i32::MAX),
            Some(action) if self.active_tab == Tab::Containers => self.on_containers_action(action),
            Some(action) if self.active_tab == Tab::Snapshots => self.on_snapshots_action(action),
            Some(AppAction::New) if self.active_tab == Tab::Images => self.open_new_form(),
            Some(AppAction::EditAlias) if self.active_tab == Tab::Images => self.open_alias_form(),
            Some(AppAction::Download) if self.active_tab == Tab::Images => self.open_image_catalog(),
            Some(AppAction::Delete) if self.active_tab == Tab::Images => self.confirm_delete_image(),
            Some(AppAction::Info) if self.active_tab == Tab::Images => self.open_image_info(),
            Some(AppAction::Export) if self.active_tab == Tab::Images => self.open_export_image_form(),
            Some(AppAction::Import) if self.active_tab == Tab::Images => self.open_import_image_form(),
            Some(AppAction::New) if self.active_tab == Tab::Profiles => self.open_new_profile_form(),
            Some(AppAction::Delete) if self.active_tab == Tab::Profiles => self.confirm_delete_profile(),
            Some(AppAction::EditDescription) if self.active_tab == Tab::Profiles => {
                self.open_edit_profile_description_form();
            }
            Some(AppAction::Info) if self.active_tab == Tab::Profiles => self.open_profile_show(),
            Some(AppAction::Limits) if self.active_tab == Tab::Profiles => self.open_profile_limits_form(),
            Some(action) if self.active_tab == Tab::Storage => self.on_storage_action(action),
            Some(AppAction::New) if self.active_tab == Tab::Network => self.open_new_network_form(),
            Some(AppAction::Attach) if self.active_tab == Tab::Network => self.open_attach_network_form(),
            Some(AppAction::Delete) if self.active_tab == Tab::Network => self.confirm_delete_network(),
            Some(AppAction::ConfigKeys) if self.active_tab == Tab::Network => self.open_network_config_keys_form(),
            Some(AppAction::Info) if self.active_tab == Tab::Network => self.open_network_info(),
            Some(AppAction::Operations) if self.active_tab == Tab::Logs => {
                self.open_info(i18n::t(self.lang, Key::InfoTitleOperations).to_string(), PendingAction::FetchOperations);
            }
            Some(AppAction::Warnings) if self.active_tab == Tab::Logs => {
                self.open_info(i18n::t(self.lang, Key::InfoTitleWarnings).to_string(), PendingAction::FetchWarnings);
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: i32) {
        let len = match self.active_tab {
            Tab::Containers => self.filtered_instances().len(),
            Tab::Snapshots => self
                .selected_instance()
                .map(|i| i.snapshots.len())
                .unwrap_or(0),
            Tab::Images => self.inventory.images.len(),
            Tab::Profiles => self.inventory.profiles.len(),
            Tab::Network => self.inventory.networks.len(),
            Tab::Storage => match &self.storage_drill {
                Some(pool) => self.pool_volumes(pool).len(),
                None => self.inventory.pools.len(),
            },
            _ => return,
        };
        let row = match self.active_tab {
            Tab::Containers => &mut self.containers_row,
            Tab::Snapshots => &mut self.snapshot_row,
            Tab::Images => &mut self.image_row,
            Tab::Profiles => &mut self.profile_row,
            Tab::Network => &mut self.network_row,
            Tab::Storage if self.storage_drill.is_some() => &mut self.storage_volume_row,
            Tab::Storage => &mut self.storage_row,
            _ => return,
        };
        if len == 0 {
            *row = 0;
            return;
        }
        *row = match delta {
            i32::MIN => 0,
            i32::MAX => len - 1,
            -1 => row.saturating_sub(1),
            1 => (*row + 1).min(len - 1),
            _ => *row,
        };
        if self.active_tab == Tab::Containers {
            self.selected_instance_name = self
                .filtered_instances()
                .get(self.containers_row)
                .map(|i| i.name.clone());
        }
    }

    fn on_containers_action(&mut self, action: AppAction) {
        // `Import` crea una instancia nueva desde un backup: no necesita
        // ninguna seleccionada, a diferencia de todo lo demás en esta tabla.
        if action == AppAction::Import {
            self.open_import_backup_form();
            return;
        }

        let Some(instance) = self.selected_instance().cloned() else {
            return;
        };
        let name = instance.name.clone();

        match action {
            AppAction::Start => self.pending = Some(PendingAction::Start(name)),
            AppAction::Stop => self.pending = Some(PendingAction::Stop(name)),
            AppAction::Restart => self.pending = Some(PendingAction::Restart(name)),
            AppAction::PauseToggle => {
                self.pending = Some(match instance.status {
                    InstanceStatus::Frozen => PendingAction::Start(name),
                    InstanceStatus::Running => PendingAction::Pause(name),
                    _ => {
                        self.toast = Some((i18n::t(self.lang, Key::ToastPauseInvalid).to_string(), Status::Warning));
                        return;
                    }
                });
            }
            AppAction::New => self.open_new_form(),
            AppAction::Clone => {
                self.form = Some(ActionForm::new(
                    FormKind::Clone,
                    name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelDestName)],
                    vec![format!("{name}-copy")],
                ));
            }
            AppAction::Move => {
                self.form = Some(ActionForm::new(
                    FormKind::Rename,
                    name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelNewName)],
                    vec![name.clone()],
                ));
            }
            AppAction::Snapshot => {
                self.form = Some(ActionForm::new(
                    FormKind::Snapshot,
                    name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelSnapName)],
                    vec![chrono::Local::now().format("snap-%Y%m%d-%H%M%S").to_string()],
                ));
            }
            AppAction::Limits => {
                self.form = Some(ActionForm::new(
                    FormKind::Limits,
                    name.clone(),
                    vec![
                        i18n::t(self.lang, Key::FormLabelLimitsCpu),
                        i18n::t(self.lang, Key::FormLabelLimitsMemory),
                    ],
                    vec![
                        instance.limits_cpu.clone().unwrap_or_default(),
                        instance.limits_memory.clone().unwrap_or_default(),
                    ],
                ));
            }
            AppAction::ConfigKeys => {
                self.form = Some(ActionForm::new(
                    FormKind::ConfigKeys,
                    name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelConfigKeys)],
                    vec![String::new()],
                ));
            }
            AppAction::Export => {
                let filename = format!("{name}-{}.tar.gz", chrono::Local::now().format("%Y%m%d-%H%M%S"));
                // Sugiere guardarlo en `backups_dir()` — es lo que
                // `list_backup_files` (selector de `Import`) va a mirar; si
                // el usuario cambia la ruta a mano, esa carpeta simplemente
                // no aparece en el selector la próxima vez.
                let default_path = crate::lxc::backups_dir()
                    .map(|dir| dir.join(&filename).to_string_lossy().into_owned())
                    .unwrap_or(filename);
                self.form = Some(ActionForm::new(
                    FormKind::ExportInstance,
                    name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelBackupPath)],
                    vec![default_path],
                ));
            }
            AppAction::Console => self.pending = Some(PendingAction::Console(name)),
            AppAction::Unset => {
                self.form = Some(ActionForm::new(
                    FormKind::UnsetConfigKeys,
                    name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelUnsetKeys)],
                    vec![String::new()],
                ));
            }
            AppAction::Files => {
                self.form = Some(
                    ActionForm::new(
                        FormKind::TransferFile,
                        name.clone(),
                        vec![
                            i18n::t(self.lang, Key::FormLabelTransferDirection),
                            i18n::t(self.lang, Key::FormLabelLocalPath),
                            i18n::t(self.lang, Key::FormLabelInstancePath),
                            i18n::t(self.lang, Key::FormLabelRecursive),
                        ],
                        vec!["push".to_string(), String::new(), "/root/".to_string(), "n".to_string()],
                    )
                    .with_select(0, vec!["push".to_string(), "pull".to_string()]),
                );
            }
            AppAction::Devices => {
                self.form = Some(
                    ActionForm::new(
                        FormKind::Device,
                        name.clone(),
                        vec![
                            i18n::t(self.lang, Key::FormLabelDeviceAction),
                            i18n::t(self.lang, Key::FormLabelDeviceId),
                            i18n::t(self.lang, Key::FormLabelDeviceParams),
                        ],
                        vec!["proxy".to_string(), String::new(), String::new()],
                    )
                    .with_select(0, DEVICE_ACTIONS.iter().map(|a| a.to_string()).collect()),
                );
            }
            AppAction::Publish => {
                self.form = Some(ActionForm::new(
                    FormKind::PublishInstance,
                    name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelPublishAlias)],
                    vec![String::new()],
                ));
            }
            AppAction::ViewLogs => {
                self.active_tab = Tab::Logs;
                self.fetch_log();
            }
            AppAction::Info => {
                self.open_info(name.clone(), PendingAction::FetchInfo(name));
            }
            AppAction::AssignProfiles => {
                self.form = Some(ActionForm::new(
                    FormKind::AssignProfiles,
                    name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelProfiles)],
                    vec![instance.profiles.join(",")],
                ));
            }
            AppAction::Delete => {
                self.confirm = Some(ConfirmState {
                    action: ConfirmAction::DeleteInstance(name.clone()),
                    expected: name,
                    input: String::new(),
                    message: i18n::t(self.lang, Key::ConfirmDeleteInstance).to_string(),
                });
            }
            _ => {}
        }
    }

    fn on_snapshots_action(&mut self, action: AppAction) {
        let Some(instance) = self.selected_instance().cloned() else {
            return;
        };
        let Some(snap) = instance.snapshots.get(self.snapshot_row) else {
            if action == AppAction::Snapshot {
                self.form = Some(ActionForm::new(
                    FormKind::Snapshot,
                    instance.name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelSnapName)],
                    vec![chrono::Local::now().format("snap-%Y%m%d-%H%M%S").to_string()],
                ));
            }
            return;
        };
        let snap_name = snap.name.clone();

        match action {
            AppAction::Snapshot => {
                self.form = Some(ActionForm::new(
                    FormKind::Snapshot,
                    instance.name.clone(),
                    vec![i18n::t(self.lang, Key::FormLabelSnapName)],
                    vec![chrono::Local::now().format("snap-%Y%m%d-%H%M%S").to_string()],
                ));
            }
            AppAction::Restart => {
                self.confirm = Some(ConfirmState {
                    action: ConfirmAction::RestoreSnapshot {
                        name: instance.name.clone(),
                        snap: snap_name,
                    },
                    expected: instance.name.clone(),
                    input: String::new(),
                    message: i18n::t(self.lang, Key::ConfirmRestoreSnapshot).to_string(),
                });
            }
            AppAction::Delete => {
                self.confirm = Some(ConfirmState {
                    action: ConfirmAction::DeleteSnapshot {
                        name: instance.name.clone(),
                        snap: snap_name,
                    },
                    expected: instance.name.clone(),
                    input: String::new(),
                    message: i18n::t(self.lang, Key::ConfirmDeleteSnapshot).to_string(),
                });
            }
            _ => {}
        }
    }

    fn open_new_form(&mut self) {
        let default_image = self
            .inventory
            .images
            .get(self.image_row)
            .and_then(|img| img.aliases.first())
            .map(|a| a.name.clone())
            .unwrap_or_default();

        self.form = Some(
            ActionForm::new(
                FormKind::NewInstance,
                String::new(),
                vec![
                    i18n::t(self.lang, Key::FormLabelImageAlias),
                    i18n::t(self.lang, Key::FormLabelName),
                    i18n::t(self.lang, Key::FormLabelStartNow),
                    i18n::t(self.lang, Key::FormLabelInstanceType),
                    i18n::t(self.lang, Key::FormLabelLaunchProfiles),
                    i18n::t(self.lang, Key::FormLabelLaunchCpu),
                    i18n::t(self.lang, Key::FormLabelLaunchMemory),
                    i18n::t(self.lang, Key::FormLabelLaunchRootSize),
                ],
                vec![
                    default_image,
                    String::new(),
                    "s".to_string(),
                    "container".to_string(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ],
            )
            .with_select(3, vec!["container".to_string(), "vm".to_string()]),
        );
    }

    fn open_alias_form(&mut self) {
        let Some(img) = self.inventory.images.get(self.image_row) else {
            return;
        };
        let current_alias = img.aliases.first().map(|a| a.name.clone()).unwrap_or_default();
        self.form = Some(ActionForm::new(
            FormKind::Alias,
            img.fingerprint.clone(),
            vec![i18n::t(self.lang, Key::FormLabelNewAlias)],
            vec![current_alias],
        ));
    }

    fn open_image_catalog(&mut self) {
        self.image_catalog = Some(ImageCatalogState { loading: true, ..Default::default() });
        self.pending = Some(PendingAction::FetchRemoteImages);
    }

    /// Devuelve `true` si el modal debe seguir abierto (mismo contrato que
    /// `on_confirm_key`): se saca el estado de `self.image_catalog` antes de
    /// llamar para poder tocar `self.pending` sin pelear con el borrow checker.
    fn on_image_catalog_key(&mut self, state: &mut ImageCatalogState, key: KeyEvent) -> bool {
        use crossterm::event::KeyCode;

        if state.filter_active {
            match key.code {
                KeyCode::Esc => {
                    state.filter_active = false;
                    state.filter.clear();
                    state.row = 0;
                }
                KeyCode::Enter => state.filter_active = false,
                KeyCode::Backspace => {
                    state.filter.pop();
                }
                KeyCode::Char(c) => state.filter.push(c),
                _ => {}
            }
            state.row = state.row.min(state.filtered().len().saturating_sub(1));
            return true;
        }

        match key.code {
            KeyCode::Esc => return false,
            KeyCode::Char('/') => state.filter_active = true,
            KeyCode::Char('r') | KeyCode::Char('R') => {
                state.loading = true;
                state.error = None;
                self.pending = Some(PendingAction::FetchRemoteImages);
            }
            KeyCode::Up | KeyCode::Char('k') => state.row = state.row.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                let len = state.filtered().len();
                if len > 0 {
                    state.row = (state.row + 1).min(len - 1);
                }
            }
            KeyCode::Home | KeyCode::Char('g') => state.row = 0,
            KeyCode::End | KeyCode::Char('G') => {
                state.row = state.filtered().len().saturating_sub(1);
            }
            KeyCode::Enter => {
                if let Some(entry) = state.filtered().get(state.row).copied() {
                    let alias = entry.image.aliases.first().map(|a| a.name.clone());
                    let target = alias.clone().unwrap_or_else(|| entry.image.fingerprint.clone());
                    let reference = format!("{}:{target}", entry.remote);
                    // La descarga corre en segundo plano y puede tardar; sin este
                    // aviso inmediato, la barra queda muda hasta que termine y
                    // parece que `Enter` no hizo nada.
                    self.toast = Some((
                        i18n::t(self.lang, Key::ToastImageDownloadStartedTemplate).replacen("{}", &target, 1),
                        Status::Info,
                    ));
                    self.pending = Some(PendingAction::DownloadImage { reference, alias });
                }
                return false;
            }
            _ => {}
        }
        true
    }

    fn open_new_profile_form(&mut self) {
        self.form = Some(ActionForm::new(
            FormKind::NewProfile,
            String::new(),
            vec![i18n::t(self.lang, Key::FormLabelName), i18n::t(self.lang, Key::FormLabelDescription)],
            vec![String::new(), String::new()],
        ));
    }

    /// Abre el popup de sólo lectura y pide su contenido. `target` es lo que
    /// rotula el popup (y lo que valida que una respuesta tardía siga siendo la
    /// del popup abierto); `fetch` es también lo que re-pide `R`.
    fn open_info(&mut self, target: String, fetch: PendingAction) {
        self.info_open = true;
        self.info_loading = true;
        self.info_text = None;
        self.info_target = Some(target);
        self.info_scroll = 0;
        self.info_refresh = Some(fetch.clone());
        self.pending = Some(fetch);
    }

    fn open_profile_show(&mut self) {
        let Some(profile) = self.inventory.profiles.get(self.profile_row) else {
            return;
        };
        let name = profile.name.clone();
        self.open_info(name.clone(), PendingAction::FetchProfileShow(name));
    }

    fn open_profile_limits_form(&mut self) {
        let Some(profile) = self.inventory.profiles.get(self.profile_row) else {
            return;
        };
        // `profile list` no trae `limits.cpu`/`limits.memory` (sólo
        // name/description/used_by) — a diferencia del form de límites de
        // instancia, acá no hay valor previo para precargar.
        self.form = Some(ActionForm::new(
            FormKind::ProfileLimits,
            profile.name.clone(),
            vec![i18n::t(self.lang, Key::FormLabelLimitsCpu), i18n::t(self.lang, Key::FormLabelLimitsMemory)],
            vec![String::new(), String::new()],
        ));
    }

    fn open_edit_profile_description_form(&mut self) {
        let Some(profile) = self.inventory.profiles.get(self.profile_row) else {
            return;
        };
        self.form = Some(ActionForm::new(
            FormKind::EditProfileDescription,
            profile.name.clone(),
            vec![i18n::t(self.lang, Key::FormLabelDescription)],
            vec![profile.description.clone()],
        ));
    }

    fn selected_pool(&self) -> Option<&StoragePool> {
        self.inventory.pools.get(self.storage_row)
    }

    /// Volúmenes de `pool`, en el mismo orden en que vienen del inventario —
    /// se recalcula en cada llamado (lista chica, no vale la pena cachear) en
    /// vez de guardarse aparte, así nunca queda desincronizada del último poll.
    fn pool_volumes(&self, pool: &str) -> Vec<&VolumeEntry> {
        self.inventory.volumes.iter().filter(|v| v.pool == pool).collect()
    }

    fn selected_volume(&self) -> Option<&VolumeEntry> {
        let pool = self.storage_drill.as_deref()?;
        self.pool_volumes(pool).into_iter().nth(self.storage_volume_row)
    }

    fn drill_into_pool(&mut self) {
        let Some(pool) = self.selected_pool() else {
            return;
        };
        self.storage_drill = Some(pool.name.clone());
        self.storage_volume_row = 0;
    }

    /// El pool "activo" para `i`/`n`/`Supr` a nivel STORAGE: el del drill si
    /// hay uno, si no el de la fila seleccionada en la tabla de pools.
    fn active_pool_name(&self) -> Option<String> {
        self.storage_drill.clone().or_else(|| self.selected_pool().map(|p| p.name.clone()))
    }

    fn on_storage_action(&mut self, action: AppAction) {
        match action {
            AppAction::New => {
                if self.storage_drill.is_some() {
                    self.open_new_volume_form();
                } else {
                    self.open_new_pool_form();
                }
            }
            AppAction::Delete => {
                if self.storage_drill.is_some() {
                    self.confirm_delete_volume();
                } else {
                    self.confirm_delete_pool();
                }
            }
            AppAction::Attach if self.storage_drill.is_some() => self.open_attach_volume_form(),
            AppAction::Detach if self.storage_drill.is_some() => self.open_detach_volume_form(),
            // `Move`/`ConfigKeys` se reusan acá con el mismo sentido que en
            // CONTAINERS (renombrar / `config set` libre), sólo que sobre el
            // volumen seleccionado en vez de una instancia.
            AppAction::Move if self.storage_drill.is_some() => self.open_rename_volume_form(),
            AppAction::ConfigKeys if self.storage_drill.is_some() => self.open_volume_config_keys_form(),
            AppAction::Info if self.storage_drill.is_some() => self.open_volume_info(),
            AppAction::Snapshot if self.storage_drill.is_some() => self.open_snapshot_volume_form(),
            AppAction::MoveTo if self.storage_drill.is_some() => self.open_move_volume_form(),
            AppAction::Info => self.open_storage_show(),
            AppAction::Enter => self.drill_into_pool(),
            AppAction::Back => {
                self.storage_drill = None;
                self.storage_volume_row = 0;
            }
            _ => {}
        }
    }

    fn open_new_pool_form(&mut self) {
        self.form = Some(
            ActionForm::new(
                FormKind::NewPool,
                String::new(),
                vec![
                    i18n::t(self.lang, Key::FormLabelName),
                    i18n::t(self.lang, Key::FormLabelDriver),
                    i18n::t(self.lang, Key::FormLabelPoolSource),
                ],
                vec![String::new(), STORAGE_DRIVERS[0].to_string(), String::new()],
            )
            .with_select(1, STORAGE_DRIVERS.iter().map(|d| d.to_string()).collect()),
        );
    }

    fn confirm_delete_pool(&mut self) {
        let Some(pool) = self.selected_pool() else {
            return;
        };
        let name = pool.name.clone();
        self.confirm = Some(ConfirmState {
            action: ConfirmAction::DeletePool(name.clone()),
            expected: name,
            input: String::new(),
            message: i18n::t(self.lang, Key::ConfirmDeletePool).to_string(),
        });
    }

    fn open_storage_show(&mut self) {
        let Some(pool) = self.active_pool_name() else {
            return;
        };
        self.open_info(pool.clone(), PendingAction::FetchStorageInfo(pool));
    }

    fn open_new_volume_form(&mut self) {
        let default_pool = self.active_pool_name().unwrap_or_default();
        self.form = Some(ActionForm::new(
            FormKind::NewVolume,
            String::new(),
            vec![
                i18n::t(self.lang, Key::FormLabelStoragePool),
                i18n::t(self.lang, Key::FormLabelName),
                i18n::t(self.lang, Key::FormLabelVolumeSize),
            ],
            vec![default_pool, String::new(), String::new()],
        ));
    }

    fn confirm_delete_volume(&mut self) {
        let Some(vol) = self.selected_volume() else {
            return;
        };
        let pool = vol.pool.clone();
        let name = vol.name.clone();
        self.confirm = Some(ConfirmState {
            action: ConfirmAction::DeleteVolume { pool, name: name.clone() },
            expected: name,
            input: String::new(),
            message: i18n::t(self.lang, Key::ConfirmDeleteVolume).to_string(),
        });
    }

    /// Nombres de instancia existentes, para el selector cíclico (`◀ ▶`) de
    /// los forms de adjuntar/desvincular volumen — tipear el nombre a mano
    /// invita a typos que LXD sólo reporta al correr el comando.
    fn instance_names(&self) -> Vec<String> {
        self.instances.iter().map(|i| i.name.clone()).collect()
    }

    /// `target` del form guarda `<pool>/<nombre>` — un volumen se identifica
    /// por ambos, y el form genérico sólo tiene un slot de `target` (igual que
    /// `Clone`/`Snapshot` usan el nombre de instancia).
    fn open_attach_volume_form(&mut self) {
        let Some(vol) = self.selected_volume() else {
            return;
        };
        let default_path = format!("/mnt/{}", vol.name);
        let instances = self.instance_names();
        let default_instance = instances.first().cloned().unwrap_or_default();
        self.form = Some(
            ActionForm::new(
                FormKind::AttachVolume,
                format!("{}/{}", vol.pool, vol.name),
                vec![i18n::t(self.lang, Key::FormLabelInstanceName), i18n::t(self.lang, Key::FormLabelMountPath)],
                vec![default_instance, default_path],
            )
            .with_select(0, instances),
        );
    }

    /// Precarga la primera instancia de `used_by` (si hay alguna, y sigue
    /// existiendo) — es el caso más común: desvincular de la única instancia
    /// que lo tiene adjunto.
    fn open_detach_volume_form(&mut self) {
        let Some(vol) = self.selected_volume() else {
            return;
        };
        let instances = self.instance_names();
        let used_by = vol.used_by.first().and_then(|url| url.rsplit('/').next()).unwrap_or_default().to_string();
        let default_instance = if instances.contains(&used_by) {
            used_by
        } else {
            instances.first().cloned().unwrap_or_default()
        };
        self.form = Some(
            ActionForm::new(
                FormKind::DetachVolume,
                format!("{}/{}", vol.pool, vol.name),
                vec![i18n::t(self.lang, Key::FormLabelInstanceName)],
                vec![default_instance],
            )
            .with_select(0, instances),
        );
    }

    fn open_rename_volume_form(&mut self) {
        let Some(vol) = self.selected_volume() else {
            return;
        };
        self.form = Some(ActionForm::new(
            FormKind::RenameVolume,
            format!("{}/{}", vol.pool, vol.name),
            vec![i18n::t(self.lang, Key::FormLabelNewName)],
            vec![vol.name.clone()],
        ));
    }

    fn open_volume_config_keys_form(&mut self) {
        let Some(vol) = self.selected_volume() else {
            return;
        };
        self.form = Some(ActionForm::new(
            FormKind::VolumeConfigKeys,
            format!("{}/{}", vol.pool, vol.name),
            vec![i18n::t(self.lang, Key::FormLabelConfigKeys)],
            vec![String::new()],
        ));
    }

    fn open_import_backup_form(&mut self) {
        let backups = crate::lxc::list_backup_files();
        let default_path = backups.first().cloned().unwrap_or_default();
        self.form = Some(
            ActionForm::new(
                FormKind::ImportBackup,
                String::new(),
                vec![i18n::t(self.lang, Key::FormLabelImportPath)],
                vec![default_path],
            )
            .with_select(0, backups),
        );
    }

    fn selected_network(&self) -> Option<&crate::lxc::NetworkInfo> {
        self.inventory.networks.get(self.network_row)
    }

    fn open_new_network_form(&mut self) {
        self.form = Some(ActionForm::new(
            FormKind::NewNetwork,
            String::new(),
            vec![i18n::t(self.lang, Key::FormLabelName), i18n::t(self.lang, Key::FormLabelConfigKeysOptional)],
            vec![String::new(), String::new()],
        ));
    }

    fn open_attach_network_form(&mut self) {
        let Some(net) = self.selected_network() else {
            return;
        };
        let instances = self.instance_names();
        let default_instance = instances.first().cloned().unwrap_or_default();
        self.form = Some(
            ActionForm::new(
                FormKind::AttachNetwork,
                net.name.clone(),
                vec![i18n::t(self.lang, Key::FormLabelInstanceName), i18n::t(self.lang, Key::FormLabelDeviceName)],
                vec![default_instance, String::new()],
            )
            .with_select(0, instances),
        );
    }

    fn open_volume_info(&mut self) {
        let Some(vol) = self.selected_volume() else {
            return;
        };
        let pool = vol.pool.clone();
        let volume = crate::lxc::actions::volume_ref(&vol.kind, &vol.name);
        self.open_info(format!("{pool}/{volume}"), PendingAction::FetchVolumeInfo { pool, volume });
    }

    /// `target` = `<pool>/<ref>`, donde `<ref>` lleva el tipo si no es `custom`
    /// (`container/web01`): `lxc storage volume snapshot|move` lo necesitan para
    /// no confundir un volumen de instancia con uno personalizado homónimo.
    fn open_snapshot_volume_form(&mut self) {
        let Some(vol) = self.selected_volume() else {
            return;
        };
        let target = format!("{}/{}", vol.pool, crate::lxc::actions::volume_ref(&vol.kind, &vol.name));
        self.form = Some(ActionForm::new(
            FormKind::SnapshotVolume,
            target,
            vec![i18n::t(self.lang, Key::FormLabelSnapNameOptional)],
            vec![chrono::Local::now().format("snap-%Y%m%d-%H%M%S").to_string()],
        ));
    }

    fn open_move_volume_form(&mut self) {
        let Some(vol) = self.selected_volume() else {
            return;
        };
        let others: Vec<String> =
            self.inventory.pools.iter().filter(|p| p.name != vol.pool).map(|p| p.name.clone()).collect();
        let target = format!("{}/{}", vol.pool, crate::lxc::actions::volume_ref(&vol.kind, &vol.name));
        let default = others.first().cloned().unwrap_or_default();
        self.form = Some(
            ActionForm::new(
                FormKind::MoveVolume,
                target,
                vec![i18n::t(self.lang, Key::FormLabelDestPool)],
                vec![default],
            )
            .with_select(0, others),
        );
    }

    fn confirm_delete_network(&mut self) {
        let Some(net) = self.selected_network() else {
            return;
        };
        let name = net.name.clone();
        self.confirm = Some(ConfirmState {
            action: ConfirmAction::DeleteNetwork(name.clone()),
            expected: name,
            input: String::new(),
            message: i18n::t(self.lang, Key::ConfirmDeleteNetwork).to_string(),
        });
    }

    fn open_network_config_keys_form(&mut self) {
        let Some(net) = self.selected_network() else {
            return;
        };
        self.form = Some(ActionForm::new(
            FormKind::NetworkConfigKeys,
            net.name.clone(),
            vec![i18n::t(self.lang, Key::FormLabelConfigKeys)],
            vec![String::new()],
        ));
    }

    fn open_network_info(&mut self) {
        let Some(net) = self.selected_network() else {
            return;
        };
        let name = net.name.clone();
        self.open_info(name.clone(), PendingAction::FetchNetworkInfo(name));
    }

    /// Alias de la imagen seleccionada, o su fingerprint corto si no tiene —
    /// lo que `lxc image info|export` aceptan como referencia y lo que el
    /// usuario reconoce en la tabla.
    fn selected_image_ref(&self) -> Option<String> {
        let img = self.inventory.images.get(self.image_row)?;
        Some(
            img.aliases
                .first()
                .map(|a| a.name.clone())
                .unwrap_or_else(|| img.fingerprint.chars().take(12).collect()),
        )
    }

    fn open_image_info(&mut self) {
        let Some(image) = self.selected_image_ref() else {
            return;
        };
        self.open_info(image.clone(), PendingAction::FetchImageInfo(image));
    }

    fn open_export_image_form(&mut self) {
        let Some(image) = self.selected_image_ref() else {
            return;
        };
        let default_dir = crate::lxc::backups_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        self.form = Some(ActionForm::new(
            FormKind::ExportImage,
            image,
            vec![i18n::t(self.lang, Key::FormLabelExportDir)],
            vec![default_dir],
        ));
    }

    fn open_import_image_form(&mut self) {
        self.form = Some(ActionForm::new(
            FormKind::ImportImage,
            String::new(),
            vec![i18n::t(self.lang, Key::FormLabelImageFile), i18n::t(self.lang, Key::FormLabelNewAliasOptional)],
            vec![String::new(), String::new()],
        ));
    }

    fn confirm_delete_image(&mut self) {
        let Some(img) = self.inventory.images.get(self.image_row) else {
            return;
        };
        let label = img
            .aliases
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| img.fingerprint.chars().take(12).collect());
        self.confirm = Some(ConfirmState {
            action: ConfirmAction::DeleteImage(img.fingerprint.clone()),
            expected: label,
            input: String::new(),
            message: i18n::t(self.lang, Key::ConfirmDeleteImage).to_string(),
        });
    }

    fn confirm_delete_profile(&mut self) {
        let Some(profile) = self.inventory.profiles.get(self.profile_row) else {
            return;
        };
        let name = profile.name.clone();
        self.confirm = Some(ConfirmState {
            action: ConfirmAction::DeleteProfile(name.clone()),
            expected: name,
            input: String::new(),
            message: i18n::t(self.lang, Key::ConfirmDeleteProfile).to_string(),
        });
    }

    fn fetch_log(&mut self) {
        if let Some(name) = self.selected_instance_name.clone() {
            self.log_loading = true;
            self.pending = Some(PendingAction::FetchLog(name));
        }
    }

    fn on_filter_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => {
                self.filter_active = false;
                self.filter_input.clear();
                self.containers_row = 0;
            }
            KeyCode::Enter => self.filter_active = false,
            KeyCode::Backspace => {
                self.filter_input.pop();
            }
            KeyCode::Char(c) => self.filter_input.push(c),
            _ => {}
        }
        self.containers_row = self.containers_row.min(
            self.filtered_instances()
                .len()
                .saturating_sub(1),
        );
    }

    /// ↑↓ (y Pg Up/Dn, Home/End) desplazan el modal de ayuda; cualquier otra
    /// tecla lo cierra — mismo contrato que `on_info_key`, es de sólo lectura.
    fn on_help_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        let max_scroll = crate::ui::widgets::help::line_count(self.lang).saturating_sub(1);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.help_scroll = self.help_scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.help_scroll = (self.help_scroll + 1).min(max_scroll);
            }
            KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(10),
            KeyCode::PageDown => {
                self.help_scroll = self.help_scroll.saturating_add(10).min(max_scroll);
            }
            KeyCode::Home | KeyCode::Char('g') => self.help_scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.help_scroll = max_scroll,
            _ => {
                self.help_open = false;
                self.help_scroll = 0;
            }
        }
    }

    /// Cuántas líneas tiene el texto cargado — cota para el scroll (no
    /// considera wrapping, que depende del ancho real de la terminal; el
    /// widget vuelve a acotar contra el alto visible al dibujar).
    fn info_line_count(&self) -> u16 {
        self.info_text.as_ref().map(|t| t.lines().count().max(1) as u16).unwrap_or(1)
    }

    /// Popup de `lxc info` + `lxc config show`: arriba/abajo (y Pg Up/Dn,
    /// Home/End) navegan el texto, `r`/`R` re-pide ambos comandos, cualquier
    /// otra tecla lo cierra — es de sólo lectura, no hace falta distinguir Esc
    /// de otra cosa salvo las de scroll.
    fn on_info_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        let max_scroll = self.info_line_count().saturating_sub(1);
        match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Some(fetch) = self.info_refresh.clone() {
                    self.info_loading = true;
                    self.info_scroll = 0;
                    self.pending = Some(fetch);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.info_scroll = self.info_scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.info_scroll = (self.info_scroll + 1).min(max_scroll);
            }
            KeyCode::PageUp => self.info_scroll = self.info_scroll.saturating_sub(10),
            KeyCode::PageDown => {
                self.info_scroll = self.info_scroll.saturating_add(10).min(max_scroll);
            }
            KeyCode::Home | KeyCode::Char('g') => self.info_scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.info_scroll = max_scroll,
            _ => {
                self.info_open = false;
                self.info_text = None;
                self.info_target = None;
                self.info_refresh = None;
                self.info_scroll = 0;
            }
        }
    }

    fn on_config_key(&mut self, key: KeyEvent) {
        let Some(form) = self.config_form.as_mut() else {
            return;
        };
        match form.on_key(key) {
            ConfigOutcome::None => {}
            ConfigOutcome::Cancel => self.config_form = None,
            ConfigOutcome::Save(config) => match config.save() {
                Ok(_) => {
                    self.config = (*config).clone();
                    self.lang = self.config.ui.language;
                    self.system_check.set_lang(self.lang);
                    let _ = self.reload.config.send((*config).clone());
                    self.config_form = None;
                    self.toast = Some((i18n::t(self.lang, Key::ConfigSaved).to_string(), Status::Ok));
                }
                Err(e) => {
                    let lang = self.lang;
                    if let Some(form) = self.config_form.as_mut() {
                        form.error =
                            Some(i18n::t(lang, Key::ConfigSaveFailedTemplate).replacen("{}", &e.to_string(), 1));
                    }
                }
            },
        }
    }

    /// Devuelve `true` si el modal debe seguir abierto (el llamador reinserta
    /// `confirm` en `self.confirm`), `false` si se cerró (cancelado o confirmado).
    fn on_confirm_key(&mut self, confirm: &mut ConfirmState, key: KeyEvent) -> bool {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Esc => false,
            KeyCode::Backspace => {
                confirm.input.pop();
                true
            }
            KeyCode::Enter => {
                if confirm.input == confirm.expected {
                    self.pending = Some(match confirm.action.clone() {
                        ConfirmAction::DeleteInstance(name) => PendingAction::DeleteInstance(name),
                        ConfirmAction::RestoreSnapshot { name, snap } => {
                            PendingAction::Restore { name, snap }
                        }
                        ConfirmAction::DeleteSnapshot { name, snap } => {
                            PendingAction::DeleteSnapshot { name, snap }
                        }
                        ConfirmAction::DeleteProfile(name) => PendingAction::DeleteProfile(name),
                        ConfirmAction::DeleteImage(fingerprint) => PendingAction::DeleteImage(fingerprint),
                        ConfirmAction::DeleteVolume { pool, name } => {
                            PendingAction::DeleteVolume { pool, name }
                        }
                        ConfirmAction::DeletePool(name) => PendingAction::DeletePool(name),
                        ConfirmAction::DeleteNetwork(name) => PendingAction::DeleteNetwork(name),
                    });
                    false
                } else {
                    self.toast = Some((i18n::t(self.lang, Key::ToastNameMismatch).to_string(), Status::Error));
                    true
                }
            }
            KeyCode::Char(c) => {
                confirm.input.push(c);
                true
            }
            _ => true,
        }
    }

    fn on_form_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyCode;
        let Some(form) = self.form.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.form = None;
                return;
            }
            KeyCode::Tab | KeyCode::Down => {
                form.focus = (form.focus + 1) % form.labels.len();
                return;
            }
            KeyCode::Up => {
                form.focus = (form.focus + form.labels.len() - 1) % form.labels.len();
                return;
            }
            KeyCode::Backspace => {
                form.values[form.focus].pop();
                return;
            }
            KeyCode::Left | KeyCode::Right => {
                // Clona la lista para no pelear con el borrow checker al
                // escribir `form.values[form.focus]` dos líneas más abajo —
                // son listas chicas (drivers, nombres de instancia), no vale
                // la pena complicar el préstamo por evitar el clone.
                if let Some(options) = form.select_options[form.focus].clone() {
                    let current =
                        options.iter().position(|o| *o == form.values[form.focus]).unwrap_or(0);
                    let len = options.len();
                    let next = if key.code == KeyCode::Left {
                        (current + len - 1) % len
                    } else {
                        (current + 1) % len
                    };
                    form.values[form.focus] = options[next].clone();
                }
                return;
            }
            KeyCode::Char(c) => {
                form.values[form.focus].push(c);
                return;
            }
            KeyCode::Enter => {}
            _ => return,
        }

        // Enter: validar y despachar según el tipo de formulario.
        let form = self.form.take().expect("chequeado arriba");
        match form.kind {
            FormKind::NewInstance => {
                let image = form.values[0].trim().to_string();
                let name = form.values[1].trim().to_string();
                if image.is_empty() || name.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrImageNameRequired)));
                    return;
                }
                let start = !form.values[2].trim().eq_ignore_ascii_case("n");
                let mut config = Vec::new();
                if let Some(cpu) = non_empty(&form.values[5]) {
                    config.push(("limits.cpu".to_string(), cpu));
                }
                if let Some(memory) = non_empty(&form.values[6]) {
                    config.push(("limits.memory".to_string(), memory));
                }
                let opts = LaunchOptions {
                    vm: form.values[3].trim() == "vm",
                    profiles: split_list(&form.values[4]),
                    config,
                    root_size: non_empty(&form.values[7]),
                };
                self.pending = Some(PendingAction::Launch { image, name, start, opts });
            }
            FormKind::Clone => {
                let dst = form.values[0].trim().to_string();
                if dst.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrDestNameRequired)));
                    return;
                }
                self.pending = Some(PendingAction::Copy {
                    src: form.target.clone(),
                    dst,
                });
            }
            FormKind::Rename => {
                let new_name = form.values[0].trim().to_string();
                if new_name.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrNewNameRequired)));
                    return;
                }
                if new_name == form.target {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrNewNameUnchanged)));
                    return;
                }
                // Optimista: si el `move` falla, el próximo poll trae la
                // lista real y corrige la selección; no vale la pena esperar
                // la respuesta async sólo para esto.
                if self.selected_instance_name.as_deref() == Some(form.target.as_str()) {
                    self.selected_instance_name = Some(new_name.clone());
                }
                self.pending = Some(PendingAction::Rename { old: form.target.clone(), new: new_name });
            }
            FormKind::ConfigKeys => {
                let Some(pairs) = parse_pairs(&form.values[0]) else {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrConfigKeysInvalid)));
                    return;
                };
                if pairs.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrConfigKeysRequired)));
                    return;
                }
                self.pending = Some(PendingAction::SetConfigKeys { name: form.target.clone(), pairs });
            }
            FormKind::VolumeConfigKeys => {
                let Some(pairs) = parse_pairs(&form.values[0]) else {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrConfigKeysInvalid)));
                    return;
                };
                if pairs.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrConfigKeysRequired)));
                    return;
                }
                let Some((pool, name)) = form.target.split_once('/') else {
                    return;
                };
                let (pool, name) = (pool.to_string(), name.to_string());
                self.pending = Some(PendingAction::SetVolumeConfigKeys { pool, name, pairs });
            }
            FormKind::RenameVolume => {
                let new_name = form.values[0].trim().to_string();
                if new_name.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrNewNameRequired)));
                    return;
                }
                let Some((pool, old)) = form.target.split_once('/') else {
                    return;
                };
                let (pool, old) = (pool.to_string(), old.to_string());
                if new_name == old {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrNewNameUnchanged)));
                    return;
                }
                self.pending = Some(PendingAction::RenameVolume { pool, old, new: new_name });
            }
            FormKind::ExportInstance => {
                let path = form.values[0].trim().to_string();
                if path.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrBackupPathRequired)));
                    return;
                }
                self.pending = Some(PendingAction::ExportInstance { name: form.target.clone(), path });
            }
            FormKind::ImportBackup => {
                let path = form.values[0].trim().to_string();
                if path.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrBackupPathRequired)));
                    return;
                }
                self.pending = Some(PendingAction::ImportBackup { path });
            }
            FormKind::NewNetwork => {
                let name = form.values[0].trim().to_string();
                if name.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrNetworkNameRequired)));
                    return;
                }
                let Some(pairs) = parse_pairs(&form.values[1]) else {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrConfigKeysInvalid)));
                    return;
                };
                self.pending = Some(PendingAction::CreateNetwork { name, pairs });
            }
            FormKind::AttachNetwork => {
                let instance = form.values[0].trim().to_string();
                if instance.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrInstanceNameRequired)));
                    return;
                }
                let device = non_empty(&form.values[1]);
                self.pending =
                    Some(PendingAction::AttachNetwork { network: form.target.clone(), instance, device });
            }
            FormKind::Snapshot => {
                let snap = form.values[0].trim().to_string();
                if snap.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrSnapNameRequired)));
                    return;
                }
                self.pending = Some(PendingAction::Snapshot {
                    name: form.target.clone(),
                    snap,
                });
            }
            FormKind::Limits => {
                let cpu = non_empty(&form.values[0]);
                let memory = non_empty(&form.values[1]);
                self.pending = Some(PendingAction::SetLimits {
                    name: form.target.clone(),
                    cpu,
                    memory,
                });
            }
            FormKind::ProfileLimits => {
                let cpu = non_empty(&form.values[0]);
                let memory = non_empty(&form.values[1]);
                self.pending = Some(PendingAction::SetProfileLimits {
                    name: form.target.clone(),
                    cpu,
                    memory,
                });
            }
            FormKind::Alias => {
                let new_alias = form.values[0].trim().to_string();
                if new_alias.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrAliasRequired)));
                    return;
                }
                let old_alias = self
                    .inventory
                    .images
                    .iter()
                    .find(|img| img.fingerprint == form.target)
                    .and_then(|img| img.aliases.first())
                    .map(|a| a.name.clone());
                self.pending = Some(PendingAction::SetImageAlias {
                    fingerprint: form.target.clone(),
                    old_alias,
                    new_alias,
                });
            }
            FormKind::NewProfile => {
                let name = form.values[0].trim().to_string();
                if name.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrProfileNameRequired)));
                    return;
                }
                let description = non_empty(&form.values[1]);
                self.pending = Some(PendingAction::CreateProfile { name, description });
            }
            FormKind::AssignProfiles => {
                let profiles: Vec<String> = form.values[0]
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                self.pending = Some(PendingAction::AssignProfiles {
                    name: form.target.clone(),
                    profiles,
                });
            }
            FormKind::NewPool => {
                let name = form.values[0].trim().to_string();
                let driver = form.values[1].trim().to_string();
                if name.is_empty() || driver.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrPoolDriverRequired)));
                    return;
                }
                let source = non_empty(&form.values[2]);
                self.pending = Some(PendingAction::CreatePool { name, driver, source });
            }
            FormKind::NewVolume => {
                let pool = form.values[0].trim().to_string();
                let name = form.values[1].trim().to_string();
                if pool.is_empty() || name.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrPoolNameRequired)));
                    return;
                }
                let size = non_empty(&form.values[2]);
                self.pending = Some(PendingAction::CreateVolume { pool, name, size });
            }
            FormKind::AttachVolume => {
                let instance = form.values[0].trim().to_string();
                if instance.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrInstanceNameRequired)));
                    return;
                }
                let path = form.values[1].trim().to_string();
                if path.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrMountPathRequired)));
                    return;
                }
                let Some((pool, name)) = form.target.split_once('/') else {
                    return;
                };
                self.pending = Some(PendingAction::AttachVolume {
                    pool: pool.to_string(),
                    name: name.to_string(),
                    instance,
                    path,
                });
            }
            FormKind::DetachVolume => {
                let instance = form.values[0].trim().to_string();
                if instance.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrInstanceNameRequired)));
                    return;
                }
                let Some((pool, name)) = form.target.split_once('/') else {
                    return;
                };
                self.pending = Some(PendingAction::DetachVolume {
                    pool: pool.to_string(),
                    name: name.to_string(),
                    instance,
                });
            }
            FormKind::UnsetConfigKeys => {
                let keys = split_list(&form.values[0]);
                if keys.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrKeysRequired)));
                    return;
                }
                self.pending = Some(PendingAction::UnsetConfigKeys { name: form.target.clone(), keys });
            }
            FormKind::TransferFile => {
                let local = expand_tilde(form.values[1].trim());
                let remote = form.values[2].trim().to_string();
                if local.is_empty() || remote.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrPathsRequired)));
                    return;
                }
                let recursive = matches!(form.values[3].trim().to_lowercase().as_str(), "s" | "si" | "y" | "yes");
                self.pending = Some(PendingAction::TransferFile {
                    name: form.target.clone(),
                    push: form.values[0].trim() != "pull",
                    local,
                    remote,
                    recursive,
                });
            }
            FormKind::Device => {
                let action = form.values[0].trim().to_string();
                let device = form.values[1].trim().to_string();
                let params = form.values[2].trim().to_string();
                if action == "remove" {
                    if device.is_empty() {
                        self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrDeviceNameRequired)));
                        return;
                    }
                    self.pending = Some(PendingAction::RemoveDevice { name: form.target.clone(), device });
                    return;
                }
                let pairs = if action == "proxy" { proxy_pairs(&params) } else { parse_pairs(&params) };
                let required: [&str; 2] = if action == "proxy" { ["listen", "connect"] } else { ["source", "path"] };
                let valid = pairs
                    .as_ref()
                    .is_some_and(|p| required.iter().all(|k| p.iter().any(|(key, _)| key == k)));
                let Some(pairs) = pairs.filter(|_| valid) else {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrDeviceParamsInvalid)));
                    return;
                };
                // Un proxy con el atajo de puertos no necesita nombre: se lo
                // deriva del puerto publicado.
                let device = if device.is_empty() && action == "proxy" {
                    let port = pairs
                        .iter()
                        .find(|(k, _)| k == "listen")
                        .and_then(|(_, v)| v.rsplit(':').next())
                        .unwrap_or("0");
                    format!("proxy-{port}")
                } else {
                    device
                };
                if device.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrDeviceNameRequired)));
                    return;
                }
                self.pending = Some(PendingAction::AddDevice { name: form.target.clone(), device, kind: action, pairs });
            }
            FormKind::NetworkConfigKeys => {
                let Some(pairs) = parse_pairs(&form.values[0]) else {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrConfigKeysInvalid)));
                    return;
                };
                if pairs.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrConfigKeysRequired)));
                    return;
                }
                self.pending = Some(PendingAction::SetNetworkKeys { name: form.target.clone(), pairs });
            }
            FormKind::PublishInstance => {
                self.pending = Some(PendingAction::PublishInstance {
                    name: form.target.clone(),
                    alias: non_empty(&form.values[0]),
                });
            }
            FormKind::ExportImage => {
                let dir = expand_tilde(form.values[0].trim());
                if dir.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrPathsRequired)));
                    return;
                }
                self.pending = Some(PendingAction::ExportImage { image: form.target.clone(), dir });
            }
            FormKind::ImportImage => {
                let path = expand_tilde(form.values[0].trim());
                if path.is_empty() {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrBackupPathRequired)));
                    return;
                }
                self.pending = Some(PendingAction::ImportImage { path, alias: non_empty(&form.values[1]) });
            }
            FormKind::SnapshotVolume => {
                let Some((pool, volume)) = form.target.split_once('/') else {
                    return;
                };
                self.pending = Some(PendingAction::SnapshotVolume {
                    pool: pool.to_string(),
                    volume: volume.to_string(),
                    snap: non_empty(&form.values[0]),
                });
            }
            FormKind::MoveVolume => {
                let dest_pool = form.values[0].trim().to_string();
                let Some((pool, volume)) = form.target.split_once('/') else {
                    return;
                };
                if dest_pool.is_empty() || dest_pool == pool {
                    self.form = Some(with_error(form, i18n::t(self.lang, Key::FormErrDestPoolRequired)));
                    return;
                }
                self.pending = Some(PendingAction::MoveVolume {
                    pool: pool.to_string(),
                    volume: volume.to_string(),
                    dest_pool,
                });
            }
            FormKind::EditProfileDescription => {
                let description = form.values[0].trim().to_string();
                self.pending = Some(PendingAction::SetProfileDescription {
                    name: form.target.clone(),
                    description,
                });
            }
        }
    }
}

fn with_error(mut form: ActionForm, message: &str) -> ActionForm {
    form.error = Some(message.to_string());
    form
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Parsea un campo `clave=valor,clave2=valor2` — usado por `ConfigKeys`,
/// `VolumeConfigKeys` y la config opcional de `NewNetwork`. `None` = algún
/// par no tenía `=` o la clave venía vacía; `Some(vec![])` es válido (campo
/// vacío) y cada caller decide si eso es error o no.
fn parse_pairs(raw: &str) -> Option<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    for pair in raw.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match pair.split_once('=') {
            Some((k, v)) if !k.trim().is_empty() => pairs.push((k.trim().to_string(), v.trim().to_string())),
            _ => return None,
        }
    }
    Some(pairs)
}

/// Acciones del formulario de devices (`FormKind::Device`): `proxy` publica un
/// puerto, `disk` monta una carpeta del host, `remove` quita un device.
const DEVICE_ACTIONS: [&str; 3] = ["proxy", "disk", "remove"];

/// Parte una lista `a,b,c` en items sin espacios ni vacíos.
fn split_list(raw: &str) -> Vec<String> {
    raw.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect()
}

/// `~/ruta` -> `$HOME/ruta`: el comando se lanza sin shell, así que la tilde
/// no se expande sola.
fn expand_tilde(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{}/{rest}", home.trim_end_matches('/')),
        _ => path.to_string(),
    }
}

/// Parámetros de un device `proxy`: atajo `PUERTO_HOST:PUERTO_INSTANCIA`
/// (TCP, escucha en todas las interfaces) o pares `clave=valor` completos.
fn proxy_pairs(raw: &str) -> Option<Vec<(String, String)>> {
    if let Some((host, inner)) = raw.split_once(':')
        && !host.is_empty()
        && !inner.is_empty()
        && host.chars().all(|c| c.is_ascii_digit())
        && inner.chars().all(|c| c.is_ascii_digit())
    {
        return Some(vec![
            ("listen".to_string(), format!("tcp:0.0.0.0:{host}")),
            ("connect".to_string(), format!("tcp:127.0.0.1:{inner}")),
        ]);
    }
    parse_pairs(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_acepta_el_atajo_de_puertos() {
        assert_eq!(
            proxy_pairs("8080:80"),
            Some(vec![
                ("listen".to_string(), "tcp:0.0.0.0:8080".to_string()),
                ("connect".to_string(), "tcp:127.0.0.1:80".to_string()),
            ])
        );
    }

    #[test]
    fn proxy_acepta_pares_completos_y_rechaza_basura() {
        let pairs = proxy_pairs("listen=udp:0.0.0.0:53,connect=udp:127.0.0.1:53").unwrap();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, "listen");
        assert!(proxy_pairs("no-es-un-par").is_none());
    }

    #[test]
    fn split_list_descarta_vacios() {
        assert_eq!(split_list(" default, webserver ,,"), vec!["default".to_string(), "webserver".to_string()]);
        assert!(split_list("").is_empty());
    }

    #[test]
    fn tilde_solo_se_expande_al_inicio() {
        assert_eq!(expand_tilde("/tmp/a"), "/tmp/a");
        assert_eq!(expand_tilde("a~/b"), "a~/b");
    }
}
