use super::cli::{self, Cli, LxcError};
use super::{
    CatalogImage, ImageInfo, Instance, Inventory, NetworkInfo, ProfileInfo, RawInstance,
    RawStorageVolume, ServerInfo, StoragePool, VolumeEntry,
};

pub async fn server_info(cli_bin: Cli, timeout_secs: u64) -> Result<ServerInfo, LxcError> {
    cli::run_json(cli_bin, &["query", "/1.0"], timeout_secs).await
}

pub async fn list_instances(cli_bin: Cli, timeout_secs: u64) -> Result<Vec<Instance>, LxcError> {
    let raw: Vec<RawInstance> =
        cli::run_json(cli_bin, &["list", "--format", "json"], timeout_secs).await?;
    Ok(raw.into_iter().map(Instance::from).collect())
}

pub async fn list_networks(cli_bin: Cli, timeout_secs: u64) -> Result<Vec<NetworkInfo>, LxcError> {
    cli::run_json(cli_bin, &["network", "list", "--format", "json"], timeout_secs).await
}

pub async fn list_images(cli_bin: Cli, timeout_secs: u64) -> Result<Vec<ImageInfo>, LxcError> {
    cli::run_json(cli_bin, &["image", "list", "--format", "json"], timeout_secs).await
}

/// Catálogo de un remote (`lxc image list <remote>: --format json`) — bajo
/// demanda desde el modal de descarga de IMAGES, nunca por el poller: es una
/// llamada de red al remote, no una lectura local del daemon.
pub async fn list_remote_images(
    cli_bin: Cli,
    remote: &str,
    timeout_secs: u64,
) -> Result<Vec<ImageInfo>, LxcError> {
    let target = format!("{remote}:");
    cli::run_json(cli_bin, &["image", "list", &target, "--format", "json"], timeout_secs).await
}

/// Junta el catálogo de todos los `remotes` (`DOWNLOAD_REMOTES`) en una sola
/// lista. Un remote caído no tira a los demás: sólo se propaga error si
/// *ninguno* respondió — con al menos un remote vivo, el catálogo se arma con
/// lo que sí llegó.
pub async fn list_remote_catalog(
    cli_bin: Cli,
    remotes: &[&'static str],
    timeout_secs: u64,
) -> Result<Vec<CatalogImage>, LxcError> {
    let mut catalog = Vec::new();
    let mut last_err = None;
    for remote in remotes {
        match list_remote_images(cli_bin, remote, timeout_secs).await {
            Ok(images) => {
                catalog.extend(images.into_iter().map(|image| CatalogImage { remote, image }))
            }
            Err(e) => last_err = Some(e),
        }
    }
    if catalog.is_empty()
        && let Some(e) = last_err
    {
        return Err(e);
    }
    Ok(catalog)
}

pub async fn list_storage_pools(
    cli_bin: Cli,
    timeout_secs: u64,
) -> Result<Vec<StoragePool>, LxcError> {
    cli::run_json(cli_bin, &["storage", "list", "--format", "json"], timeout_secs).await
}

pub async fn list_profiles(cli_bin: Cli, timeout_secs: u64) -> Result<Vec<ProfileInfo>, LxcError> {
    cli::run_json(cli_bin, &["profile", "list", "--format", "json"], timeout_secs).await
}

/// `lxc storage show <pool>` — YAML de config del pool (driver, source,
/// config) para el popup de la pestaña STORAGE (tecla `i`).
pub async fn storage_show(cli_bin: Cli, pool: &str, timeout_secs: u64) -> Result<String, LxcError> {
    let raw = cli::run(cli_bin, &["storage", "show", pool], timeout_secs).await?;
    Ok(String::from_utf8_lossy(&raw).to_string())
}

/// `lxc storage info <pool>` — uso de espacio del pool, la otra mitad del
/// popup de `i` junto con `storage_show` y `list_storage_volumes`.
pub async fn storage_pool_info(cli_bin: Cli, pool: &str, timeout_secs: u64) -> Result<String, LxcError> {
    let raw = cli::run(cli_bin, &["storage", "info", pool], timeout_secs).await?;
    Ok(String::from_utf8_lossy(&raw).to_string())
}

/// `lxc storage volume list <pool> --format json` — volúmenes de un pool
/// puntual; LXD/Incus no tienen un comando que liste los de todos los pools
/// juntos, hay que pedirlos pool por pool (`list_all_volumes`).
pub async fn list_storage_volumes(
    cli_bin: Cli,
    pool: &str,
    timeout_secs: u64,
) -> Result<Vec<RawStorageVolume>, LxcError> {
    cli::run_json(cli_bin, &["storage", "volume", "list", pool, "--format", "json"], timeout_secs).await
}

/// Junta los volúmenes de todos los `pools` en una sola lista, etiquetados con
/// su pool de origen — mismo criterio de tolerancia que `list_remote_catalog`:
/// un pool que falla no tira a los demás, el error (si lo hubo) se devuelve
/// aparte para que el caller decida si banderearlo sin vaciar lo que sí llegó.
pub async fn list_all_volumes(
    cli_bin: Cli,
    pools: &[StoragePool],
    timeout_secs: u64,
) -> (Vec<VolumeEntry>, Option<LxcError>) {
    let mut volumes = Vec::new();
    let mut last_err = None;
    for pool in pools {
        match list_storage_volumes(cli_bin, &pool.name, timeout_secs).await {
            Ok(raw) => volumes.extend(raw.into_iter().map(|v| VolumeEntry {
                pool: pool.name.clone(),
                name: v.name,
                kind: v.kind,
                content_type: v.content_type,
                used_by: v.used_by,
            })),
            Err(e) => last_err = Some(e),
        }
    }
    (volumes, last_err)
}

/// `lxc info <name> --show-log` — log de arranque del contenedor (consola inicial,
/// systemd, etc.), distinto del log de la propia TUI.
pub async fn instance_log(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<String, LxcError> {
    let raw = cli::run(cli_bin, &["info", name, "--show-log"], timeout_secs).await?;
    Ok(String::from_utf8_lossy(&raw).to_string())
}

/// `lxc info <name>` — estado detallado de la instancia (IPs, procesos, recursos,
/// snapshots) para el popup de la pestaña CONTAINERS, distinto de `instance_log`
/// (que sólo trae `--show-log`, la consola de arranque).
pub async fn instance_info(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<String, LxcError> {
    let raw = cli::run(cli_bin, &["info", name], timeout_secs).await?;
    Ok(String::from_utf8_lossy(&raw).to_string())
}

/// `lxc config show <name> --expanded` — YAML de la config efectiva de la
/// instancia: la propia más lo heredado de los perfiles (devices, límites,
/// etc.), la otra mitad del popup de `i` junto con `instance_info`. Sin
/// `--expanded` los devices y límites que vienen de un perfil no aparecen.
pub async fn instance_config_show(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<String, LxcError> {
    text(cli_bin, &["config", "show", name, "--expanded"], timeout_secs).await
}

/// Corre un comando y devuelve su stdout como texto, para los popups de sólo
/// lectura.
async fn text(cli_bin: Cli, args: &[&str], timeout_secs: u64) -> Result<String, LxcError> {
    let raw = cli::run(cli_bin, args, timeout_secs).await?;
    Ok(String::from_utf8_lossy(&raw).to_string())
}

/// `lxc network show <name>` — config de la red (popup de `i` en NETWORK).
pub async fn network_show(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<String, LxcError> {
    text(cli_bin, &["network", "show", name], timeout_secs).await
}

/// `lxc network list-leases <name>` — concesiones DHCP de la red. Sólo existe
/// en redes gestionadas con DHCP propio; en las demás el comando falla y el
/// popup muestra el error en su sección.
pub async fn network_leases(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<String, LxcError> {
    text(cli_bin, &["network", "list-leases", name], timeout_secs).await
}

/// `lxc image info <fingerprint>` — detalle de una imagen local.
pub async fn image_info(cli_bin: Cli, fingerprint: &str, timeout_secs: u64) -> Result<String, LxcError> {
    text(cli_bin, &["image", "info", fingerprint], timeout_secs).await
}

/// `lxc storage volume show <pool> <volumen>` — config de un volumen.
pub async fn volume_show(cli_bin: Cli, pool: &str, volume: &str, timeout_secs: u64) -> Result<String, LxcError> {
    text(cli_bin, &["storage", "volume", "show", pool, volume], timeout_secs).await
}

/// `lxc operation list` — operaciones en curso del daemon.
pub async fn operation_list(cli_bin: Cli, timeout_secs: u64) -> Result<String, LxcError> {
    text(cli_bin, &["operation", "list"], timeout_secs).await
}

/// `lxc warning list` — advertencias que el daemon tiene registradas.
pub async fn warning_list(cli_bin: Cli, timeout_secs: u64) -> Result<String, LxcError> {
    text(cli_bin, &["warning", "list"], timeout_secs).await
}

/// `lxc profile show <name>` — YAML completo del perfil (config, devices,
/// descripción) para el popup de la pestaña PROFILES (tecla `i`).
pub async fn profile_show(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<String, LxcError> {
    let raw = cli::run(cli_bin, &["profile", "show", name], timeout_secs).await?;
    Ok(String::from_utf8_lossy(&raw).to_string())
}

/// Las cuatro lecturas de baja frecuencia combinadas: si alguna falla se reporta
/// el primer error y se dejan las demás en blanco antes que tirar todo el poll.
pub async fn inventory(cli_bin: Cli, timeout_secs: u64) -> (Inventory, Option<LxcError>) {
    let mut inventory = Inventory::default();
    let mut first_error = None;

    match server_info(cli_bin, timeout_secs).await {
        Ok(s) => inventory.server = Some(s),
        Err(e) => { first_error.get_or_insert(e); }
    }
    match list_networks(cli_bin, timeout_secs).await {
        Ok(n) => inventory.networks = n,
        Err(e) => { first_error.get_or_insert(e); }
    }
    match list_images(cli_bin, timeout_secs).await {
        Ok(i) => inventory.images = i,
        Err(e) => { first_error.get_or_insert(e); }
    }
    match list_storage_pools(cli_bin, timeout_secs).await {
        Ok(pools) => {
            let (volumes, vol_err) = list_all_volumes(cli_bin, &pools, timeout_secs).await;
            inventory.pools = pools;
            inventory.volumes = volumes;
            if let Some(e) = vol_err {
                first_error.get_or_insert(e);
            }
        }
        Err(e) => { first_error.get_or_insert(e); }
    }
    match list_profiles(cli_bin, timeout_secs).await {
        Ok(p) => inventory.profiles = p,
        Err(e) => { first_error.get_or_insert(e); }
    }

    (inventory, first_error)
}
