pub mod actions;
pub mod cli;
pub mod fixtures;
pub mod poller;
pub mod query;

use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;

/// Remotes de catálogo público consultados por la pestaña IMAGES para listar
/// imágenes disponibles para descargar (`lxc image list <remote>: --format
/// json`). Todos preconfigurados de fábrica por lxc/incus — sin selector de
/// remotes adicional ni remotes propios del usuario, sólo estos tres.
pub const DOWNLOAD_REMOTES: &[&str] = &["images", "ubuntu", "ubuntu-daily"];

/// Piso de timeout para llamadas de red al remote (catálogo y `image copy`):
/// `command_timeout_secs` del usuario está pensado para comandos locales
/// (arranque/parada/etc, típicamente 10s) y una imagen puede tardar minutos en
/// transferirse — con el timeout corto, `run()` reporta error pero el proceso
/// hijo sigue corriendo de fondo y la imagen aparece igual segundos/minutos
/// después, dejando un aviso de error falso. Se usa el máximo entre este piso
/// y lo configurado, nunca menos.
pub const DOWNLOAD_TIMEOUT_SECS: u64 = 300;

/// Opciones extra de `lxc launch`/`lxc init` más allá de imagen y nombre: tipo
/// de instancia, perfiles, config inicial (`limits.*`) y tamaño del disco raíz.
/// `Default` = contenedor con el perfil `default` y sin overrides, que es el
/// comportamiento histórico de la TUI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchOptions {
    /// `--vm`: máquina virtual en vez de contenedor.
    pub vm: bool,
    /// `--profile` repetido. Ojo: indicar alguno reemplaza al perfil `default`
    /// (hay que listarlo explícitamente si se lo quiere); vacío = no se manda
    /// ningún flag y LXD aplica `default` solo.
    pub profiles: Vec<String>,
    /// `-c clave=valor` repetido (`limits.cpu`, `limits.memory`, ...).
    pub config: Vec<(String, String)>,
    /// `-d root,size=<tamaño>`: override del disco raíz del perfil.
    pub root_size: Option<String>,
}

/// Drivers de storage soportados por `lxc storage create <nombre> <driver>`
/// (LXD/Incus) — los cinco más comunes más las dos variantes de Ceph
/// (filesystem/object), no drivers exóticos. El form de creación de pool
/// (`FormKind::NewPool`) los ofrece como selector cíclico (`◀ ▶`), no texto
/// libre: un driver mal tipeado falla recién al correr el comando.
pub const STORAGE_DRIVERS: &[&str] = &["dir", "btrfs", "zfs", "lvm", "ceph", "cephfs", "cephobject"];

/// Carpeta donde `FormKind::ExportInstance` sugiere guardar los backups y de
/// donde `list_backup_files` los lista para el selector de `FormKind::
/// ImportBackup` — mismo mecanismo (`directories::ProjectDirs`) que usa
/// `config.rs` para el archivo de config, pero en `data_dir()` (son datos
/// generados, no configuración).
pub fn backups_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("", "", "voxcontainer-tui").map(|d| d.data_dir().join("backups"))
}

/// Backups (`.tar.gz`) encontrados en `backups_dir()`, más recientes primero.
/// Vacío (no error) si la carpeta no existe todavía — pasa nada, es el estado
/// normal antes del primer `lxc export`; el form de import cae solo a texto
/// libre (`ActionForm::with_select` no marca el campo como selector con una
/// lista vacía).
pub fn list_backup_files() -> Vec<String> {
    let Some(dir) = backups_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut files: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(".tar.gz")))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    // El timestamp del nombre (sugerido por `AppAction::Export`) ordena
    // cronológicamente como string — no hace falta leer metadata del archivo.
    files.sort();
    files.reverse();
    files
}

/// LXD/Incus mandan `null` (no el campo ausente, ni `{}`/`[]`) para varios mapas
/// y listas "vacíos" — típicamente `expanded_config`/`network`/`disk` en una
/// instancia detenida. `#[serde(default)]` sólo cubre el campo *ausente*: con
/// `null` presente igual falla ("invalid type: null, expected a map/sequence")
/// y tira todo el `lxc list`, dejando la pestaña CONTAINERS vacía. Este
/// deserializador trata `null` como "usar el default" también.
fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceStatus {
    Running,
    Stopped,
    Frozen,
    Starting,
    Stopping,
    Error,
    Unknown,
}

impl InstanceStatus {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "running" => Self::Running,
            "stopped" => Self::Stopped,
            "frozen" => Self::Frozen,
            "starting" => Self::Starting,
            "stopping" => Self::Stopping,
            "error" => Self::Error,
            _ => Self::Unknown,
        }
    }

    /// Texto del estado en el idioma de la interfaz (columna ESTADO).
    pub fn localized(&self, lang: crate::i18n::Lang) -> &'static str {
        use crate::i18n::{Key, t};
        t(lang, match self {
            Self::Running => Key::StatusRunning,
            Self::Stopped => Key::StatusStopped,
            Self::Frozen => Key::StatusFrozen,
            Self::Starting => Key::StatusStarting,
            Self::Stopping => Key::StatusStopping,
            Self::Error => Key::StatusError,
            Self::Unknown => Key::StatusUnknown,
        })
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Running => "RUNNING",
            Self::Stopped => "STOPPED",
            Self::Frozen => "FROZEN",
            Self::Starting => "STARTING",
            Self::Stopping => "STOPPING",
            Self::Error => "ERROR",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// `netmask` se deserializa por completitud del contrato de la API pero la TUI
/// no la pinta todavía — no hay ninguna vista que muestre subredes por interfaz.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NetworkAddress {
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub netmask: String,
    #[serde(default)]
    pub scope: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct NetworkCounters {
    #[serde(default)]
    pub bytes_received: u64,
    #[serde(default)]
    pub bytes_sent: u64,
}

/// `hwaddr`/`state` (MAC y link state por NIC) quedan reservados: no hay hoy una
/// vista de detalle por interfaz, sólo el total agregado que usa `resources`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NetworkInterfaceState {
    #[serde(default, deserialize_with = "null_default")]
    pub addresses: Vec<NetworkAddress>,
    #[serde(default)]
    pub counters: NetworkCounters,
    #[serde(default)]
    pub hwaddr: String,
    #[serde(default)]
    pub state: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CpuState {
    /// Nanosegundos de CPU consumidos desde que arrancó la instancia.
    #[serde(default)]
    pub usage: u64,
}

/// `usage_peak` queda reservado para cuando `resources` muestre el pico además
/// del uso instantáneo.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MemoryState {
    #[serde(default)]
    pub usage: u64,
    #[serde(default)]
    pub usage_peak: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DiskState {
    #[serde(default)]
    pub usage: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InstanceRuntimeState {
    #[serde(default, deserialize_with = "null_default")]
    pub network: BTreeMap<String, NetworkInterfaceState>,
    #[serde(default, deserialize_with = "null_default")]
    pub cpu: CpuState,
    #[serde(default, deserialize_with = "null_default")]
    pub memory: MemoryState,
    #[serde(default, deserialize_with = "null_default")]
    pub disk: BTreeMap<String, DiskState>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct InstanceSnapshot {
    pub name: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub stateful: bool,
}

/// Forma cruda de `lxc list --format json`. Deliberadamente parcial: el JSON real
/// trae decenas de campos y cambia entre versiones del daemon; sólo se declara lo
/// que la TUI pinta, con `#[serde(default)]` en todo lo opcional, para no romper
/// en un upgrade de LXD/Incus.
#[derive(Debug, Clone, Deserialize)]
pub struct RawInstance {
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub state: Option<InstanceRuntimeState>,
    #[serde(default, deserialize_with = "null_default")]
    pub snapshots: Vec<InstanceSnapshot>,
    #[serde(default, deserialize_with = "null_default")]
    pub expanded_config: BTreeMap<String, String>,
    #[serde(default, deserialize_with = "null_default")]
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub name: String,
    pub status: InstanceStatus,
    pub kind: String,
    pub architecture: String,
    pub created_at: String,
    pub ipv4: Vec<String>,
    pub cpu_ns: u64,
    /// Uso de CPU en porcentaje sobre el último intervalo de polling. `None`
    /// hasta la segunda lectura (hace falta un delta) — lo calcula `app.rs`.
    pub cpu_pct: Option<f64>,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    pub limits_cpu: Option<String>,
    pub limits_memory: Option<String>,
    pub snapshots: Vec<InstanceSnapshot>,
    pub profiles: Vec<String>,
}

impl From<RawInstance> for Instance {
    fn from(raw: RawInstance) -> Self {
        let ipv4 = raw
            .state
            .as_ref()
            .map(|s| {
                s.network
                    .values()
                    .flat_map(|iface| iface.addresses.iter())
                    .filter(|a| a.family == "inet" && a.scope != "link")
                    .map(|a| a.address.clone())
                    .collect()
            })
            .unwrap_or_default();

        let cpu_ns = raw.state.as_ref().map(|s| s.cpu.usage).unwrap_or(0);
        let memory_bytes = raw.state.as_ref().map(|s| s.memory.usage).unwrap_or(0);
        let disk_bytes = raw
            .state
            .as_ref()
            .map(|s| s.disk.values().map(|d| d.usage).sum())
            .unwrap_or(0);

        // Se excluye `lo`: su tráfico es interno al contenedor, no dice nada del
        // uso real de red hacia afuera.
        let (net_rx_bytes, net_tx_bytes) = raw
            .state
            .as_ref()
            .map(|s| {
                s.network
                    .iter()
                    .filter(|(name, _)| name.as_str() != "lo")
                    .fold((0u64, 0u64), |(rx, tx), (_, iface)| {
                        (
                            rx + iface.counters.bytes_received,
                            tx + iface.counters.bytes_sent,
                        )
                    })
            })
            .unwrap_or((0, 0));

        Self {
            name: raw.name,
            status: InstanceStatus::from_str(&raw.status),
            kind: if raw.kind.is_empty() {
                "container".to_string()
            } else {
                raw.kind
            },
            architecture: raw.architecture,
            created_at: raw.created_at,
            ipv4,
            cpu_ns,
            cpu_pct: None,
            memory_bytes,
            disk_bytes,
            net_rx_bytes,
            net_tx_bytes,
            limits_cpu: raw.expanded_config.get("limits.cpu").cloned(),
            limits_memory: raw.expanded_config.get("limits.memory").cloned(),
            snapshots: raw.snapshots,
            profiles: raw.profiles,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServerEnvironment {
    #[serde(default)]
    pub server_version: String,
    #[serde(default)]
    pub kernel: String,
    #[serde(default)]
    pub storage: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub api_version: String,
    #[serde(default)]
    pub environment: ServerEnvironment,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub managed: bool,
    #[serde(default, deserialize_with = "null_default")]
    pub used_by: Vec<String>,
    #[serde(default, deserialize_with = "null_default")]
    pub config: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageAlias {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageInfo {
    pub fingerprint: String,
    #[serde(default, deserialize_with = "null_default")]
    pub aliases: Vec<ImageAlias>,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub architecture: String,
    #[serde(default)]
    pub uploaded_at: String,
    /// `properties.description` es lo único de este mapa que la TUI pinta
    /// (columna DESCRIPCIÓN de `images`); el resto de las propiedades LXD
    /// (os/release/variant/...) queda sin usar.
    #[serde(default, deserialize_with = "null_default")]
    pub properties: BTreeMap<String, String>,
}

impl ImageInfo {
    pub fn description(&self) -> Option<&str> {
        self.properties.get("description").map(String::as_str).filter(|s| !s.is_empty())
    }
}

/// Entrada del catálogo remoto (modal de descarga, pestaña IMAGES) con el
/// remote de origen adjunto — necesario porque ahora se consultan varios
/// remotes (`DOWNLOAD_REMOTES`) y la referencia de descarga (`remote:alias`)
/// depende de en cuál vive cada imagen.
#[derive(Debug, Clone)]
pub struct CatalogImage {
    pub remote: &'static str,
    pub image: ImageInfo,
}

/// Forma cruda de `lxc profile list --format json`. `description` es el único
/// texto libre que la TUI edita hoy; `config`/`devices` quedan sin declarar
/// porque no hay vista que los pinte todavía.
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileInfo {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, deserialize_with = "null_default")]
    pub used_by: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StoragePool {
    pub name: String,
    #[serde(default)]
    pub driver: String,
    #[serde(default, deserialize_with = "null_default")]
    pub used_by: Vec<String>,
}

/// Forma cruda de `lxc storage volume list <pool> --format json`. El pool no
/// viaja en el JSON (es implícito en el comando) — se lo agrega `query::
/// list_all_volumes` al aplanar los volúmenes de todos los pools.
#[derive(Debug, Clone, Deserialize)]
pub struct RawStorageVolume {
    pub name: String,
    /// "custom" (volumen creado a mano), "container"/"virtual-machine" (disco
    /// raíz de una instancia), "image". La pestaña STORAGE pinta los cuatro
    /// pero sólo los `custom` tiene sentido crear/borrar/adjuntar a mano.
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub content_type: String,
    #[serde(default, deserialize_with = "null_default")]
    pub used_by: Vec<String>,
}

/// Volumen con su pool de origen adjunto — mismo criterio que `CatalogImage`
/// (imagen + remote): la pestaña STORAGE pinta una sola tabla combinando todos
/// los pools, y las acciones (crear/borrar/adjuntar/desvincular) necesitan
/// saber de qué pool viene cada fila.
#[derive(Debug, Clone)]
pub struct VolumeEntry {
    pub pool: String,
    pub name: String,
    pub kind: String,
    pub content_type: String,
    pub used_by: Vec<String>,
}

/// Instantánea combinada de todo lo que cambia poco: servidor, redes, imágenes,
/// pools y sus volúmenes. Se refresca junto porque las llamadas son baratas y de
/// baja frecuencia (`poll.inventory_secs`), a diferencia del listado de instancias.
#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub server: Option<ServerInfo>,
    pub networks: Vec<NetworkInfo>,
    pub images: Vec<ImageInfo>,
    pub pools: Vec<StoragePool>,
    pub volumes: Vec<VolumeEntry>,
    pub profiles: Vec<ProfileInfo>,
}

/// Backend resuelto una vez al arranque: qué binario correr (si hay) y si estamos
/// en modo fixtures. Se guarda en `App` y viaja con cada `PendingAction` para que
/// las acciones bajo demanda (start/stop/…) usen exactamente el mismo backend que
/// el poller, sin re-detectar el binario en cada tecla.
#[derive(Debug, Clone, Copy)]
pub struct Backend {
    pub cli: Option<cli::Cli>,
    pub fixtures: bool,
}

impl Backend {
    pub fn detect() -> Self {
        let fixtures = fixtures::enabled();
        let cli = if fixtures { None } else { cli::detect() };
        Self { cli, fixtures }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduce el error real reportado contra un daemon LXD real: `lxc list
    /// --format json` puede mandar `null` en vez de `{}`/`[]` para varios campos
    /// "vacíos" (acá `expanded_config`, `state.network`, `state.disk` de una
    /// instancia detenida) — sin `null_default` esto rompía `serde_json::from_slice`
    /// con "invalid type: null, expected a map" y dejaba CONTAINERS sin datos.
    #[test]
    fn tolera_null_en_mapas_y_listas_vacios_de_lxc_list() {
        let raw = r#"[
            {
                "name": "voxpbx-01",
                "status": "Stopped",
                "type": "container",
                "architecture": "x86_64",
                "created_at": "2026-08-01T10:00:00Z",
                "expanded_config": null,
                "profiles": null,
                "snapshots": null,
                "state": {
                    "network": null,
                    "disk": null,
                    "cpu": { "usage": 0 },
                    "memory": { "usage": 0, "usage_peak": 0 }
                }
            }
        ]"#;

        let instances: Vec<RawInstance> = serde_json::from_str(raw).expect("debe tolerar null en mapas/listas");
        assert_eq!(instances.len(), 1);
        let instance = Instance::from(instances.into_iter().next().unwrap());
        assert_eq!(instance.name, "voxpbx-01");
        assert_eq!(instance.status, InstanceStatus::Stopped);
        assert!(instance.ipv4.is_empty());
        assert!(instance.profiles.is_empty());
        assert!(instance.snapshots.is_empty());
    }
}
