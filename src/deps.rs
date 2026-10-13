use std::path::Path;
use std::process::{Command, Stdio};

use crate::i18n::{self, Key, Lang};
use crate::lxc::cli::{Cli, LxcError};

/// Chequeo y remediación de los requisitos de LXD/Incus. A diferencia de la capa
/// `lxc::cli` (async, con timeout, pensada para el poller) estas funciones corren
/// sincrónicas: se disparan sólo por tecla del usuario (abrir el wizard, `r` en
/// `SYSTEM CHECK`), nunca en el loop caliente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Dnf,
    Yum,
    Pacman,
    Apk,
    Zypper,
    Snap,
    Brew,
}

impl PackageManager {
    pub fn label(&self) -> &'static str {
        match self {
            PackageManager::Apt => "apt",
            PackageManager::Dnf => "dnf",
            PackageManager::Yum => "yum",
            PackageManager::Pacman => "pacman",
            PackageManager::Apk => "apk",
            PackageManager::Zypper => "zypper",
            PackageManager::Snap => "snap",
            PackageManager::Brew => "brew",
        }
    }

    /// Paquete que trae Incus/LXD. La mayoría de distros recientes empaquetan
    /// `incus`; donde no está en los repos (Ubuntu viejo, p.ej.) `snap install
    /// lxd` es la vía soportada oficialmente.
    fn package(&self) -> &'static str {
        match self {
            PackageManager::Snap => "lxd",
            _ => "incus",
        }
    }

    /// Comando de instalación no interactivo, con `sudo` delante salvo que ya
    /// seamos root o el gestor sea `brew` (que se niega a correr como root).
    pub fn install_command(&self) -> Vec<String> {
        let pkg = self.package().to_string();
        let base: Vec<String> = match self {
            // LXD en ambos. En Ubuntu el paquete apt `lxd` es sólo el
            // `lxd-installer`, así que se instala el snap; en Debian sí existe
            // el paquete `lxd`, y `iptables` va aparte porque el bridge lo
            // necesita y el paquete no lo arrastra.
            PackageManager::Apt => vec![
                "sh".into(),
                "-c".into(),
                ". /etc/os-release; if [ \"$ID\" = ubuntu ]; then snap install lxd; \
                 else apt-get update && apt-get install -y lxd iptables; fi"
                    .into(),
            ],
            PackageManager::Dnf => vec!["dnf".into(), "install".into(), "-y".into(), pkg],
            PackageManager::Yum => vec!["yum".into(), "install".into(), "-y".into(), pkg],
            PackageManager::Pacman => vec![
                "pacman".into(),
                "-S".into(),
                "--needed".into(),
                "--noconfirm".into(),
                pkg,
            ],
            PackageManager::Apk => vec!["apk".into(), "add".into(), "--no-cache".into(), pkg],
            PackageManager::Zypper => vec![
                "zypper".into(),
                "--non-interactive".into(),
                "install".into(),
                pkg,
            ],
            PackageManager::Snap => vec!["snap".into(), "install".into(), pkg],
            PackageManager::Brew => return vec!["brew".into(), "install".into(), pkg],
        };

        if is_root() {
            base
        } else {
            let mut with_sudo = vec!["sudo".to_string()];
            with_sudo.extend(base);
            with_sudo
        }
    }
}

/// Detecta el primer gestor de paquetes disponible. `snap` va último: si el
/// sistema tiene un gestor nativo se prefiere ese (el paquete `incus` suele
/// evitar la capa extra de snap).
pub fn detect_package_manager() -> Option<PackageManager> {
    const CANDIDATES: [(&str, PackageManager); 8] = [
        ("apt-get", PackageManager::Apt),
        ("dnf", PackageManager::Dnf),
        ("yum", PackageManager::Yum),
        ("pacman", PackageManager::Pacman),
        ("apk", PackageManager::Apk),
        ("zypper", PackageManager::Zypper),
        ("brew", PackageManager::Brew),
        ("snap", PackageManager::Snap),
    ];
    CANDIDATES
        .iter()
        .find(|(bin, _)| which(bin))
        .map(|(_, pm)| *pm)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckState {
    /// Verificado y funcionando.
    Ok(String),
    /// Falta algo y sabemos cómo arreglarlo (comando sugerido).
    Missing { detail: String, fix: Vec<String> },
    /// No se pudo determinar, o es sólo informativo (no bloquea el uso).
    Unknown(String),
}

impl CheckState {
    pub fn is_ok(&self) -> bool {
        matches!(self, CheckState::Ok(_))
    }

    pub fn detail(&self) -> &str {
        match self {
            CheckState::Ok(d) | CheckState::Unknown(d) => d,
            CheckState::Missing { detail, .. } => detail,
        }
    }

    pub fn fix(&self) -> Option<&[String]> {
        match self {
            CheckState::Missing { fix, .. } => Some(fix),
            _ => None,
        }
    }
}

/// Una fila de la ventana de verificación. `blocking` = true impide entrar al
/// tablero desde el asistente de primer arranque mientras esté en rojo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepCheck {
    pub label: &'static str,
    pub state: CheckState,
    pub blocking: bool,
}

#[derive(Debug, Clone)]
pub struct DepReport {
    pub package_manager: Option<PackageManager>,
    pub cli: Option<Cli>,
    pub checks: Vec<DepCheck>,
}

impl DepReport {
    /// Todo lo bloqueante está en verde: puede entrarse al tablero sin avisos.
    pub fn all_blocking_ok(&self) -> bool {
        self.checks
            .iter()
            .filter(|c| c.blocking)
            .all(|c| c.state.is_ok())
    }

    pub fn get(&self, index: usize) -> Option<&DepCheck> {
        self.checks.get(index)
    }
}

/// Resultado sin interpretar de correr `<cli> <args>` de forma síncrona.
enum SyncOutcome {
    Ok(serde_json::Value),
    PermissionDenied(String),
    Unreachable(String),
    Other(String),
}

fn run_sync_json(cli: Cli, args: &[&str]) -> SyncOutcome {
    let output = match Command::new(cli.binary())
        .args(args)
        .stdin(Stdio::null())
        .output()
    {
        Ok(o) => o,
        Err(e) => return SyncOutcome::Other(e.to_string()),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let lower = stderr.to_ascii_lowercase();
        if lower.contains("permission denied") {
            return SyncOutcome::PermissionDenied(stderr);
        }
        if lower.contains("connect: no such file or directory")
            || lower.contains("failed to connect")
            || lower.contains("unable to connect")
            || lower.contains("connection refused")
        {
            return SyncOutcome::Unreachable(stderr);
        }
        return SyncOutcome::Other(stderr);
    }

    match serde_json::from_slice(&output.stdout) {
        Ok(v) => SyncOutcome::Ok(v),
        Err(e) => SyncOutcome::Other(e.to_string()),
    }
}

/// Corre los seis chequeos. En macOS, sin binario, todo queda en `Unknown` con el
/// aviso de modo fixtures en vez de en rojo — así la pantalla es legible en dev.
pub fn probe(lang: Lang) -> DepReport {
    let t = |k: Key| i18n::t(lang, k);
    let cli = Cli::detect_available();
    let pm = detect_package_manager();
    let on_macos = cfg!(target_os = "macos");

    let mut checks = Vec::with_capacity(6);

    let binary_state = match cli {
        Some(c) => CheckState::Ok(t(Key::DepBinaryOkTemplate).replacen("{}", c.binary(), 1)),
        None if on_macos => CheckState::Unknown(t(Key::DepBinaryMacosFixtures).to_string()),
        None => missing(pm, LxcError::NotInstalled.localized(lang), lang),
    };
    checks.push(DepCheck {
        label: t(Key::DepBinaryLabel),
        state: binary_state,
        blocking: true,
    });

    let query = cli.map(|c| run_sync_json(c, &["query", "/1.0"]));

    let daemon_state = match (&query, on_macos) {
        (Some(SyncOutcome::Ok(_)), _) => CheckState::Ok(t(Key::DepDaemonOk).to_string()),
        (Some(SyncOutcome::Unreachable(detail)), _) => CheckState::Missing {
            detail: t(Key::DepDaemonUnreachableTemplate).replacen("{}", detail, 1),
            fix: vec![
                "sudo".into(),
                "systemctl".into(),
                "enable".into(),
                "--now".into(),
                cli.map(|c| c.service_name().to_string()).unwrap_or_default(),
            ],
        },
        (Some(SyncOutcome::PermissionDenied(_)), _) => CheckState::Unknown(t(Key::DepPermissionUnknown).to_string()),
        (Some(SyncOutcome::Other(detail)), _) => CheckState::Unknown(detail.clone()),
        (None, true) => CheckState::Unknown(t(Key::DepNotApplicableNoBinaryMacos).to_string()),
        (None, false) => CheckState::Unknown(t(Key::DepUnknownNoBinary).to_string()),
    };
    checks.push(DepCheck {
        label: t(Key::DepDaemonLabel),
        state: daemon_state,
        blocking: true,
    });

    let permission_state = match (&query, cli, on_macos) {
        (Some(SyncOutcome::Ok(_)), _, _) => CheckState::Ok(t(Key::DepPermissionOk).to_string()),
        (Some(SyncOutcome::PermissionDenied(detail)), Some(c), _) => CheckState::Missing {
            detail: detail.clone(),
            fix: vec![
                "sudo".into(),
                "usermod".into(),
                "-aG".into(),
                c.group_name().to_string(),
                std::env::var("USER").unwrap_or_else(|_| "$USER".to_string()),
            ],
        },
        (Some(SyncOutcome::Unreachable(_)), _, _) => CheckState::Unknown(t(Key::DepPermissionDependsOnDaemon).to_string()),
        (Some(SyncOutcome::Other(detail)), _, _) => CheckState::Unknown(detail.clone()),
        (None, _, true) => CheckState::Unknown(t(Key::DepNotApplicableNoBinaryMacos).to_string()),
        (None, _, false) => CheckState::Unknown(t(Key::DepUnknownNoBinary).to_string()),
        // `query` sólo puede ser `Some` si `cli` también lo es (`query = cli.map(...)`),
        // así que esta rama es inalcanzable en la práctica — sólo cierra el match.
        _ => CheckState::Unknown(t(Key::DepUnknownGeneric).to_string()),
    };
    checks.push(DepCheck {
        label: t(Key::DepPermissionLabel),
        state: permission_state,
        blocking: true,
    });

    let init_state = match (cli, on_macos) {
        (Some(c), _) => match run_sync_json(c, &["storage", "list", "--format", "json"]) {
            SyncOutcome::Ok(serde_json::Value::Array(pools)) if !pools.is_empty() => {
                CheckState::Ok(t(Key::DepInitOkTemplate).replacen("{}", &pools.len().to_string(), 1))
            }
            SyncOutcome::Ok(_) => CheckState::Missing {
                detail: t(Key::DepInitMissing).to_string(),
                fix: std::iter::once("sudo")
                    .chain(c.init_args().iter().copied())
                    .map(String::from)
                    .collect(),
            },
            _ => CheckState::Unknown(t(Key::DepUnknownGeneric).to_string()),
        },
        (None, true) => CheckState::Unknown(t(Key::DepNotApplicableNoBinaryMacos).to_string()),
        (None, false) => CheckState::Unknown(t(Key::DepUnknownNoBinary).to_string()),
    };
    checks.push(DepCheck {
        label: t(Key::DepInitLabel),
        state: init_state,
        blocking: false,
    });

    let network_state = match (cli, on_macos) {
        (Some(c), _) => match run_sync_json(c, &["network", "list", "--format", "json"]) {
            SyncOutcome::Ok(serde_json::Value::Array(nets)) => {
                let managed = nets
                    .iter()
                    .filter(|n| n.get("managed").and_then(|m| m.as_bool()).unwrap_or(false))
                    .count();
                if managed > 0 {
                    CheckState::Ok(t(Key::DepNetworkOkTemplate).replacen("{}", &managed.to_string(), 1))
                } else {
                    CheckState::Missing {
                        detail: t(Key::DepNetworkMissing).to_string(),
                        fix: vec![
                            "sudo".into(),
                            c.binary().to_string(),
                            "network".into(),
                            "create".into(),
                            c.default_bridge().to_string(),
                        ],
                    }
                }
            }
            _ => CheckState::Unknown(t(Key::DepUnknownGeneric).to_string()),
        },
        (None, true) => CheckState::Unknown(t(Key::DepNotApplicableNoBinaryMacos).to_string()),
        (None, false) => CheckState::Unknown(t(Key::DepUnknownNoBinary).to_string()),
    };
    checks.push(DepCheck {
        label: t(Key::DepNetworkLabel),
        state: network_state,
        blocking: false,
    });

    checks.push(DepCheck {
        label: t(Key::DepIdmapLabel),
        state: idmap_state(on_macos, lang),
        blocking: false,
    });

    DepReport {
        package_manager: pm,
        cli,
        checks,
    }
}

fn idmap_state(on_macos: bool, lang: Lang) -> CheckState {
    if on_macos {
        return CheckState::Unknown(i18n::t(lang, Key::DepIdmapNotApplicableMacos).to_string());
    }
    let subuid = Path::new("/etc/subuid").exists() && has_entries("/etc/subuid");
    let subgid = Path::new("/etc/subgid").exists() && has_entries("/etc/subgid");
    if subuid && subgid {
        CheckState::Ok(i18n::t(lang, Key::DepIdmapOk).to_string())
    } else {
        CheckState::Unknown(i18n::t(lang, Key::DepIdmapMissing).to_string())
    }
}

fn has_entries(path: &str) -> bool {
    std::fs::read_to_string(path)
        .map(|s| s.lines().any(|l| !l.trim().is_empty() && !l.starts_with('#')))
        .unwrap_or(false)
}

fn missing(pm: Option<PackageManager>, detail: String, lang: Lang) -> CheckState {
    match pm {
        Some(pm) => CheckState::Missing {
            detail,
            fix: pm.install_command(),
        },
        None => CheckState::Unknown(i18n::t(lang, Key::DepMissingInstallHintTemplate).replacen("{}", &detail, 1)),
    }
}

/// Ejecuta un comando de remediación heredando la terminal — hace falta para que
/// `sudo` pueda pedir la contraseña. El llamador debe haber salido de la pantalla
/// alterna de ratatui antes.
pub fn run_fix(command: &[String]) -> Result<bool, std::io::Error> {
    let Some((program, args)) = command.split_first() else {
        return Ok(false);
    };
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    Ok(status.success())
}

pub fn command_line(command: &[String]) -> String {
    command.join(" ")
}

fn which(binary: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {binary}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn is_root() -> bool {
    // SAFETY: `geteuid` no toma punteros ni puede fallar.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apt_instala_lxd_por_snap_en_ubuntu_y_por_apt_en_debian() {
        let cmd = PackageManager::Apt.install_command();
        let script = cmd.iter().find(|a| a.contains("os-release")).expect("script de apt");
        assert!(script.contains(r#"[ "$ID" = ubuntu ]; then snap install lxd"#));
        assert!(script.contains("apt-get install -y lxd iptables"));
        assert!(!script.contains("incus"));
    }
}
