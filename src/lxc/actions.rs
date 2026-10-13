use super::LaunchOptions;
use super::cli::{self, Cli, LxcError};

/// Corre `<cli> <args>` con argumentos armados dinámicamente (`Vec<String>`).
async fn run_owned(cli_bin: Cli, args: &[String], timeout_secs: u64) -> Result<(), LxcError> {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    cli::run_ok(cli_bin, &refs, timeout_secs).await
}

/// Argumentos de `lxc launch|init <imagen> <nombre> [opciones]`.
pub fn launch_args(verb: &str, image: &str, name: &str, opts: &LaunchOptions) -> Vec<String> {
    let mut args = vec![verb.to_string(), image.to_string(), name.to_string()];
    if opts.vm {
        args.push("--vm".to_string());
    }
    for profile in &opts.profiles {
        args.push("-p".to_string());
        args.push(profile.clone());
    }
    for (key, value) in &opts.config {
        args.push("-c".to_string());
        args.push(format!("{key}={value}"));
    }
    if let Some(size) = &opts.root_size {
        args.push("-d".to_string());
        args.push(format!("root,size={size}"));
    }
    args
}

/// Las rutas dentro de la instancia se mandan como `<inst><ruta>`: la ruta
/// tiene que empezar con `/` para que quede `inst/ruta`.
fn instance_path(name: &str, remote: &str) -> String {
    let remote = remote.trim();
    if remote.starts_with('/') { format!("{name}{remote}") } else { format!("{name}/{remote}") }
}

/// Argumentos de `lxc file push|pull`. `push` agrega `-p` para crear los
/// directorios intermedios del destino dentro de la instancia.
pub fn file_args(push: bool, name: &str, local: &str, remote: &str, recursive: bool) -> Vec<String> {
    let mut args = vec!["file".to_string(), if push { "push" } else { "pull" }.to_string()];
    if recursive {
        args.push("-r".to_string());
    }
    let remote = instance_path(name, remote);
    if push {
        args.push("-p".to_string());
        args.push(local.to_string());
        args.push(remote);
    } else {
        args.push(remote);
        args.push(local.to_string());
    }
    args
}

/// Argumentos de `lxc config device add <inst> <dev> <tipo> [clave=valor...]`.
pub fn device_add_args(name: &str, device: &str, kind: &str, pairs: &[(String, String)]) -> Vec<String> {
    let mut args: Vec<String> =
        ["config", "device", "add", name, device, kind].iter().map(|s| s.to_string()).collect();
    args.extend(pairs.iter().map(|(k, v)| format!("{k}={v}")));
    args
}

/// Referencia de un volumen de storage: los `custom` van sin prefijo (el tipo
/// por defecto), el resto como `<tipo>/<nombre>`.
pub fn volume_ref(kind: &str, name: &str) -> String {
    if kind.is_empty() || kind == "custom" { name.to_string() } else { format!("{kind}/{name}") }
}

pub async fn start(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["start", name], timeout_secs).await
}

pub async fn stop(cli_bin: Cli, name: &str, force: bool, timeout_secs: u64) -> Result<(), LxcError> {
    if force {
        cli::run_ok(cli_bin, &["stop", name, "--force"], timeout_secs).await
    } else {
        cli::run_ok(cli_bin, &["stop", name], timeout_secs).await
    }
}

pub async fn restart(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["restart", name], timeout_secs).await
}

/// `lxc pause` congela; el des-congelado real es `lxc start` sobre una instancia
/// `FROZEN` (LXD no tiene un verbo `unpause` separado).
pub async fn pause(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["pause", name], timeout_secs).await
}

pub async fn launch(
    cli_bin: Cli,
    image: &str,
    name: &str,
    opts: &LaunchOptions,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    run_owned(cli_bin, &launch_args("launch", image, name, opts), timeout_secs).await
}

/// Crea sin arrancar (`lxc init`) — útil cuando el usuario quiere setear límites
/// antes del primer boot.
pub async fn init_stopped(
    cli_bin: Cli,
    image: &str,
    name: &str,
    opts: &LaunchOptions,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    run_owned(cli_bin, &launch_args("init", image, name, opts), timeout_secs).await
}

pub async fn copy(cli_bin: Cli, src: &str, dst: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["copy", src, dst], timeout_secs).await
}

/// `lxc move <old> <new>` — renombra la instancia (o la mueve de storage pool
/// si `new` trae `pool:nombre`, no usado hoy). Distinto de `copy`: no deja una
/// segunda instancia, la original deja de existir con ese nombre.
pub async fn rename(cli_bin: Cli, old: &str, new: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["move", old, new], timeout_secs).await
}

/// Trae una imagen del catálogo remoto (`lxc image copy <remote>:<ref> local:`)
/// al almacén local. `alias` es el alias que traía en el remote (si tenía uno);
/// se re-asigna igual en local para que quede utilizable por nombre, no sólo
/// por fingerprint.
pub async fn download_image(
    cli_bin: Cli,
    reference: &str,
    alias: Option<&str>,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    match alias {
        Some(alias) => {
            cli::run_ok(
                cli_bin,
                &["image", "copy", reference, "local:", "--alias", alias],
                timeout_secs,
            )
            .await
        }
        None => cli::run_ok(cli_bin, &["image", "copy", reference, "local:"], timeout_secs).await,
    }
}

pub async fn snapshot(
    cli_bin: Cli,
    name: &str,
    snap: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["snapshot", name, snap], timeout_secs).await
}

pub async fn restore(
    cli_bin: Cli,
    name: &str,
    snap: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["restore", name, snap], timeout_secs).await
}

pub async fn delete_snapshot(
    cli_bin: Cli,
    name: &str,
    snap: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    let target = format!("{name}/{snap}");
    cli::run_ok(cli_bin, &["delete", &target], timeout_secs).await
}

/// Borrado forzado: si la instancia está corriendo, LXD se niega sin `--force`.
/// El caller (`app.rs`) exige que el usuario tipee el nombre antes de llegar acá.
pub async fn delete_instance(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["delete", name, "--force"], timeout_secs).await
}

/// Redefine el alias de una imagen: si ya tenía un alias distinto lo borra
/// primero (LXD no permite dos alias iguales, ni "mover" uno con un comando
/// directo) y crea el nuevo apuntando al mismo fingerprint.
pub async fn set_image_alias(
    cli_bin: Cli,
    fingerprint: &str,
    old_alias: Option<&str>,
    new_alias: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    if let Some(old) = old_alias
        && old != new_alias
    {
        cli::run_ok(cli_bin, &["image", "alias", "delete", old], timeout_secs).await?;
    }
    cli::run_ok(
        cli_bin,
        &["image", "alias", "create", new_alias, fingerprint],
        timeout_secs,
    )
    .await
}

/// Borra una imagen local por fingerprint — no por alias, que puede quedar
/// ambiguo o duplicado; el caller (`app.rs`) resuelve el fingerprint antes de
/// llegar acá y exige que el usuario tipee el nombre para confirmar.
pub async fn delete_image(cli_bin: Cli, fingerprint: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["image", "delete", fingerprint], timeout_secs).await
}

pub async fn create_profile(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["profile", "create", name], timeout_secs).await
}

pub async fn delete_profile(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["profile", "delete", name], timeout_secs).await
}

/// `lxc profile assign <instance> <p1>,<p2>,...` reemplaza la lista completa de
/// perfiles de la instancia en un solo llamado — a diferencia de `add`/`remove`,
/// no hace falta diffear contra la lista actual.
pub async fn assign_profiles(
    cli_bin: Cli,
    name: &str,
    profiles: &[String],
    timeout_secs: u64,
) -> Result<(), LxcError> {
    let joined = profiles.join(",");
    cli::run_ok(cli_bin, &["profile", "assign", name, &joined], timeout_secs).await
}

/// La `description` de un perfil es un campo de metadata, no una config key: no
/// existe un `profile set <name> description=...`. La única vía es `profile
/// edit`, que sin tty lee el YAML completo de stdin — así que hay que traer el
/// YAML actual (`profile show`), parchear sólo la línea `description:` a mano
/// (evita sumar una dependencia de YAML sólo para esto) y devolverlo.
pub async fn set_profile_description(
    cli_bin: Cli,
    name: &str,
    description: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    let raw = cli::run(cli_bin, &["profile", "show", name], timeout_secs).await?;
    let yaml = String::from_utf8_lossy(&raw).into_owned();

    let escaped = description.replace('\\', "\\\\").replace('"', "\\\"");
    let new_line = format!("description: \"{escaped}\"");

    let has_description_line = yaml.lines().any(|l| l.starts_with("description:"));
    let mut patched = if has_description_line {
        yaml.lines()
            .map(|l| if l.starts_with("description:") { new_line.as_str() } else { l })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        format!("{new_line}\n{yaml}")
    };
    patched.push('\n');

    cli::run_with_stdin(cli_bin, &["profile", "edit", name], patched.as_bytes(), timeout_secs).await?;
    Ok(())
}

pub async fn set_limits(
    cli_bin: Cli,
    name: &str,
    cpu: Option<&str>,
    memory: Option<&str>,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    if let Some(cpu) = cpu {
        let arg = format!("limits.cpu={cpu}");
        cli::run_ok(cli_bin, &["config", "set", name, &arg], timeout_secs).await?;
    }
    if let Some(memory) = memory {
        let arg = format!("limits.memory={memory}");
        cli::run_ok(cli_bin, &["config", "set", name, &arg], timeout_secs).await?;
    }
    Ok(())
}

/// `lxc config set <name> <key>=<value>` genérico — a diferencia de
/// `set_limits`, acepta cualquier clave de config (`boot.autostart`,
/// `security.nesting`, `environment.*`, etc.), no sólo límites de recursos.
/// Cada par se manda en un llamado separado: si uno falla a mitad de camino,
/// el error indica cuál, en vez de perderse en un solo comando con N flags.
pub async fn set_config_keys(
    cli_bin: Cli,
    name: &str,
    pairs: &[(String, String)],
    timeout_secs: u64,
) -> Result<(), LxcError> {
    for (key, value) in pairs {
        let arg = format!("{key}={value}");
        cli::run_ok(cli_bin, &["config", "set", name, &arg], timeout_secs).await?;
    }
    Ok(())
}

/// `lxc storage create <name> <driver> [source=<ruta>]`.
pub async fn create_pool(
    cli_bin: Cli,
    name: &str,
    driver: &str,
    source: Option<&str>,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    match source {
        Some(source) => {
            let arg = format!("source={source}");
            cli::run_ok(cli_bin, &["storage", "create", name, driver, &arg], timeout_secs).await
        }
        None => cli::run_ok(cli_bin, &["storage", "create", name, driver], timeout_secs).await,
    }
}

pub async fn delete_pool(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["storage", "delete", name], timeout_secs).await
}

/// `lxc storage volume create <pool> <name> [size=<size>]` — el tamaño es
/// opcional, si no se manda el pool usa su default.
pub async fn create_volume(
    cli_bin: Cli,
    pool: &str,
    name: &str,
    size: Option<&str>,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    match size {
        Some(size) => {
            let arg = format!("size={size}");
            cli::run_ok(cli_bin, &["storage", "volume", "create", pool, name, &arg], timeout_secs).await
        }
        None => cli::run_ok(cli_bin, &["storage", "volume", "create", pool, name], timeout_secs).await,
    }
}

pub async fn delete_volume(cli_bin: Cli, pool: &str, name: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["storage", "volume", "delete", pool, name], timeout_secs).await
}

/// `lxc storage volume attach <pool> <name> <instance> <name> <path>` — LXD
/// exige una ruta de montaje para volúmenes `custom` de tipo filesystem (el
/// default de `create_volume`); sin ella el daemon rechaza el device entero
/// ("Custom filesystem volumes require a path to be defined"). El nombre de
/// device se manda explícito (= nombre del volumen) porque, en cuanto se pasa
/// la ruta, LXD ya no lo infiere solo — `detach_volume` se apoya en ese mismo
/// nombre por default.
pub async fn attach_volume(
    cli_bin: Cli,
    pool: &str,
    name: &str,
    instance: &str,
    path: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["storage", "volume", "attach", pool, name, instance, name, path], timeout_secs).await
}

pub async fn detach_volume(
    cli_bin: Cli,
    pool: &str,
    name: &str,
    instance: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["storage", "volume", "detach", pool, name, instance], timeout_secs).await
}

/// Igual que `set_limits` pero sobre un perfil (`lxc profile set` en vez de
/// `lxc config set`) — todas las instancias que usan el perfil heredan el
/// límite, a diferencia de `set_limits` que sólo toca una instancia puntual.
pub async fn set_profile_limits(
    cli_bin: Cli,
    name: &str,
    cpu: Option<&str>,
    memory: Option<&str>,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    if let Some(cpu) = cpu {
        let arg = format!("limits.cpu={cpu}");
        cli::run_ok(cli_bin, &["profile", "set", name, &arg], timeout_secs).await?;
    }
    if let Some(memory) = memory {
        let arg = format!("limits.memory={memory}");
        cli::run_ok(cli_bin, &["profile", "set", name, &arg], timeout_secs).await?;
    }
    Ok(())
}

/// `lxc storage volume set <pool> <name> <key>=<value>` genérico — igual
/// patrón que `set_config_keys` pero sobre un volumen.
pub async fn set_volume_config_keys(
    cli_bin: Cli,
    pool: &str,
    name: &str,
    pairs: &[(String, String)],
    timeout_secs: u64,
) -> Result<(), LxcError> {
    for (key, value) in pairs {
        let arg = format!("{key}={value}");
        cli::run_ok(cli_bin, &["storage", "volume", "set", pool, name, &arg], timeout_secs).await?;
    }
    Ok(())
}

pub async fn rename_volume(
    cli_bin: Cli,
    pool: &str,
    old: &str,
    new: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["storage", "volume", "rename", pool, old, new], timeout_secs).await
}

/// `lxc export <name> <ruta>` — backup completo (config + datos) a un
/// tarball. Puede tardar minutos en instancias grandes; el caller (`main.rs`)
/// usa un timeout más generoso, igual que `download_image`.
pub async fn export_instance(
    cli_bin: Cli,
    name: &str,
    path: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    // `lxc export` no crea el directorio destino solo — el default sugerido
    // (`App::open_new...Export`, en realidad `on_containers_action`) apunta a
    // `lxc::backups_dir()`, que puede no existir todavía en el primer uso. Si
    // el usuario tipeó una ruta a mano el `create_dir_all` es un no-op sobre
    // un directorio que ya existe (o falla silencioso y deja que el `export`
    // reporte el error real).
    if let Some(parent) = std::path::Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        // Síncrono a propósito: `tokio` acá no trae el feature `fs`, y
        // crear un directorio es un `mkdir` local, no una syscall que valga
        // la pena async-ificar para un llamado que pasa una vez por export.
        let _ = std::fs::create_dir_all(parent);
    }
    cli::run_ok(cli_bin, &["export", name, path], timeout_secs).await
}

/// `lxc import <ruta>` — crea una instancia nueva a partir de un backup
/// (`export_instance`). El nombre de la instancia resultante viene del backup
/// mismo (metadata guardada al exportar), no se puede elegir acá.
pub async fn import_backup(cli_bin: Cli, path: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["import", path], timeout_secs).await
}

/// `lxc network create <name> [<key>=<value>...]`.
pub async fn create_network(
    cli_bin: Cli,
    name: &str,
    pairs: &[(String, String)],
    timeout_secs: u64,
) -> Result<(), LxcError> {
    let mut args = vec!["network".to_string(), "create".to_string(), name.to_string()];
    args.extend(pairs.iter().map(|(k, v)| format!("{k}={v}")));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    cli::run_ok(cli_bin, &args, timeout_secs).await
}

/// `lxc network attach <network> <instance> [<device>]` — sin nombre de
/// device explícito, LXD/Incus usan el nombre de la red.
pub async fn attach_network(
    cli_bin: Cli,
    network: &str,
    instance: &str,
    device: Option<&str>,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    match device {
        Some(device) => {
            cli::run_ok(cli_bin, &["network", "attach", network, instance, device], timeout_secs).await
        }
        None => cli::run_ok(cli_bin, &["network", "attach", network, instance], timeout_secs).await,
    }
}

/// `lxc config unset <name> <clave>` por cada clave — quita el valor
/// explícito de la instancia y vuelve a regir el del perfil (o el default).
pub async fn unset_config_keys(
    cli_bin: Cli,
    name: &str,
    keys: &[String],
    timeout_secs: u64,
) -> Result<(), LxcError> {
    for key in keys {
        cli::run_ok(cli_bin, &["config", "unset", name, key], timeout_secs).await?;
    }
    Ok(())
}

/// `lxc file push|pull` — copia archivos o carpetas (`-r`) entre el host y la
/// instancia. Puede tardar con árboles grandes; el caller usa el timeout largo.
pub async fn transfer_file(
    cli_bin: Cli,
    push: bool,
    name: &str,
    local: &str,
    remote: &str,
    recursive: bool,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    run_owned(cli_bin, &file_args(push, name, local, remote, recursive), timeout_secs).await
}

/// `lxc config device add <inst> <dev> <tipo> [clave=valor...]` — `proxy`
/// (publicar un puerto), `disk` (montar una carpeta del host), etc.
pub async fn add_device(
    cli_bin: Cli,
    name: &str,
    device: &str,
    kind: &str,
    pairs: &[(String, String)],
    timeout_secs: u64,
) -> Result<(), LxcError> {
    run_owned(cli_bin, &device_add_args(name, device, kind, pairs), timeout_secs).await
}

/// `lxc config device remove <inst> <dev>`.
pub async fn remove_device(cli_bin: Cli, name: &str, device: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["config", "device", "remove", name, device], timeout_secs).await
}

/// `lxc network delete <name>`.
pub async fn delete_network(cli_bin: Cli, name: &str, timeout_secs: u64) -> Result<(), LxcError> {
    cli::run_ok(cli_bin, &["network", "delete", name], timeout_secs).await
}

/// `lxc network set <name> <clave>=<valor>` por cada par.
pub async fn set_network_keys(
    cli_bin: Cli,
    name: &str,
    pairs: &[(String, String)],
    timeout_secs: u64,
) -> Result<(), LxcError> {
    for (key, value) in pairs {
        let arg = format!("{key}={value}");
        cli::run_ok(cli_bin, &["network", "set", name, &arg], timeout_secs).await?;
    }
    Ok(())
}

/// `lxc publish <instancia> [--alias <alias>]` — convierte la instancia en una
/// imagen local. LXD exige que esté detenida (o `--force`, que la apaga sola:
/// no se manda a propósito).
pub async fn publish_instance(
    cli_bin: Cli,
    name: &str,
    alias: Option<&str>,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    match alias {
        Some(alias) => cli::run_ok(cli_bin, &["publish", name, "--alias", alias], timeout_secs).await,
        None => cli::run_ok(cli_bin, &["publish", name], timeout_secs).await,
    }
}

/// `lxc image export <imagen> <directorio>` — guarda la imagen como tarball(s)
/// dentro del directorio, que se crea si no existe.
pub async fn export_image(cli_bin: Cli, image: &str, dir: &str, timeout_secs: u64) -> Result<(), LxcError> {
    let _ = std::fs::create_dir_all(dir);
    cli::run_ok(cli_bin, &["image", "export", image, dir], timeout_secs).await
}

/// `lxc image import <archivo> [--alias <alias>]`.
pub async fn import_image(
    cli_bin: Cli,
    path: &str,
    alias: Option<&str>,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    match alias {
        Some(alias) => cli::run_ok(cli_bin, &["image", "import", path, "--alias", alias], timeout_secs).await,
        None => cli::run_ok(cli_bin, &["image", "import", path], timeout_secs).await,
    }
}

/// `lxc storage volume snapshot <pool> <volumen> [<snapshot>]`.
pub async fn snapshot_volume(
    cli_bin: Cli,
    pool: &str,
    volume: &str,
    snap: Option<&str>,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    match snap {
        Some(snap) => {
            cli::run_ok(cli_bin, &["storage", "volume", "snapshot", pool, volume, snap], timeout_secs).await
        }
        None => cli::run_ok(cli_bin, &["storage", "volume", "snapshot", pool, volume], timeout_secs).await,
    }
}

/// `lxc storage volume move <pool>/<volumen> <pool-destino>/<volumen>`.
pub async fn move_volume(
    cli_bin: Cli,
    pool: &str,
    volume: &str,
    dest_pool: &str,
    timeout_secs: u64,
) -> Result<(), LxcError> {
    let src = format!("{pool}/{volume}");
    let dst = format!("{dest_pool}/{volume}");
    cli::run_ok(cli_bin, &["storage", "volume", "move", &src, &dst], timeout_secs).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn launch_sin_opciones_es_el_comando_historico() {
        assert_eq!(launch_args("launch", "ubuntu:24.04", "web01", &LaunchOptions::default()), v(&["launch", "ubuntu:24.04", "web01"]));
    }

    #[test]
    fn launch_con_todas_las_opciones() {
        let opts = LaunchOptions {
            vm: true,
            profiles: v(&["default", "webserver"]),
            config: vec![("limits.cpu".into(), "2".into()), ("limits.memory".into(), "4GiB".into())],
            root_size: Some("30GiB".into()),
        };
        assert_eq!(
            launch_args("init", "images:debian/13", "vm01", &opts),
            v(&[
                "init", "images:debian/13", "vm01", "--vm", "-p", "default", "-p", "webserver", "-c", "limits.cpu=2",
                "-c", "limits.memory=4GiB", "-d", "root,size=30GiB",
            ])
        );
    }

    #[test]
    fn file_push_crea_directorios_y_normaliza_la_ruta() {
        assert_eq!(
            file_args(true, "web01", "./app", "root/", true),
            v(&["file", "push", "-r", "-p", "./app", "web01/root/"])
        );
        assert_eq!(
            file_args(true, "web01", "a.txt", "/root/a.txt", false),
            v(&["file", "push", "-p", "a.txt", "web01/root/a.txt"])
        );
    }

    #[test]
    fn file_pull_invierte_origen_y_destino() {
        assert_eq!(
            file_args(false, "web01", ".", "/etc/hostname", false),
            v(&["file", "pull", "web01/etc/hostname", "."])
        );
    }

    #[test]
    fn device_add_arma_clave_valor() {
        let pairs = vec![
            ("listen".to_string(), "tcp:0.0.0.0:8080".to_string()),
            ("connect".to_string(), "tcp:127.0.0.1:80".to_string()),
        ];
        assert_eq!(
            device_add_args("web01", "nginx-proxy", "proxy", &pairs),
            v(&["config", "device", "add", "web01", "nginx-proxy", "proxy", "listen=tcp:0.0.0.0:8080", "connect=tcp:127.0.0.1:80"])
        );
    }

    #[test]
    fn volume_ref_solo_prefija_los_que_no_son_custom() {
        assert_eq!(volume_ref("custom", "datos"), "datos");
        assert_eq!(volume_ref("", "datos"), "datos");
        assert_eq!(volume_ref("container", "web01"), "container/web01");
    }
}
