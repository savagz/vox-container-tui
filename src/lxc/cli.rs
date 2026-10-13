use std::process::Stdio;
use std::time::Duration;

use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

/// Backend detectado: se prioriza `incus` (fork activo, mismo protocolo) y se cae
/// a `lxc` (LXD clásico) si no está. Ambos hablan el mismo JSON de API 1.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cli {
    Incus,
    Lxc,
}

impl Cli {
    pub fn binary(&self) -> &'static str {
        match self {
            Cli::Incus => "incus",
            Cli::Lxc => "lxc",
        }
    }

    pub fn service_name(&self) -> &'static str {
        match self {
            Cli::Incus => "incus",
            Cli::Lxc => "snap.lxd.daemon",
        }
    }

    pub fn group_name(&self) -> &'static str {
        match self {
            Cli::Incus => "incus",
            Cli::Lxc => "lxd",
        }
    }

    /// Comando que inicializa el daemon con valores por defecto. `lxc` es sólo
    /// el cliente de LXD: la inicialización la hace el binario `lxd`, mientras
    /// que Incus la expone como `incus admin init`.
    pub fn init_args(&self) -> &'static [&'static str] {
        match self {
            Cli::Incus => &["incus", "admin", "init", "--auto"],
            Cli::Lxc => &["lxd", "init", "--auto"],
        }
    }

    /// Nombre convencional del bridge gestionado que crea `init --auto`.
    pub fn default_bridge(&self) -> &'static str {
        match self {
            Cli::Incus => "incusbr0",
            Cli::Lxc => "lxdbr0",
        }
    }
}

#[derive(Debug, Error, Clone)]
pub enum LxcError {
    #[error("ni `incus` ni `lxc` están instalados")]
    NotInstalled,
    #[error("el daemon no responde: {0}")]
    DaemonUnreachable(String),
    #[error("permiso denegado sobre el socket: {0}")]
    PermissionDenied(String),
    #[error("`{cmd}` falló: {stderr}")]
    Command { cmd: String, stderr: String },
    #[error("no se pudo interpretar la respuesta de `{cmd}`: {detail}")]
    Parse { cmd: String, detail: String },
    #[error("`{0}` no respondió a tiempo")]
    Timeout(String),
}

impl LxcError {
    /// Igual que `Display`, pero en el idioma activo de la interfaz — `Display`
    /// (arriba) queda fijo en español porque lo usan los logs internos
    /// (`tracing`), que no son texto de la interfaz.
    pub fn localized(&self, lang: crate::i18n::Lang) -> String {
        use crate::i18n::{Key, t};
        match self {
            LxcError::NotInstalled => t(lang, Key::LxcErrNotInstalled).to_string(),
            LxcError::DaemonUnreachable(detail) => {
                t(lang, Key::LxcErrDaemonUnreachableTemplate).replacen("{}", detail, 1)
            }
            LxcError::PermissionDenied(detail) => {
                t(lang, Key::LxcErrPermissionDeniedTemplate).replacen("{}", detail, 1)
            }
            LxcError::Command { cmd, stderr } => t(lang, Key::LxcErrCommandTemplate)
                .replacen("{}", cmd, 1)
                .replacen("{}", stderr, 1),
            LxcError::Parse { cmd, detail } => t(lang, Key::LxcErrParseTemplate)
                .replacen("{}", cmd, 1)
                .replacen("{}", detail, 1),
            LxcError::Timeout(cmd) => t(lang, Key::LxcErrTimeoutTemplate).replacen("{}", cmd, 1),
        }
    }
}

/// Detecta el binario disponible, priorizando `incus` sobre `lxc` (LXD).
pub fn detect() -> Option<Cli> {
    [Cli::Incus, Cli::Lxc].into_iter().find(|c| which(c.binary()))
}

impl Cli {
    /// Alias legible para `detect()` cuando se llama desde fuera del módulo.
    pub fn detect_available() -> Option<Cli> {
        detect()
    }
}

pub fn which(binary: &str) -> bool {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {binary}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Corre `<cli> <args>` con timeout y devuelve stdout crudo. Toda lectura/escritura
/// de la capa LXC pasa por acá: es el único punto que sabe de procesos, timeouts y
/// cómo distinguir "permiso denegado" de "daemon caído" en el stderr.
pub async fn run(cli: Cli, args: &[&str], timeout_secs: u64) -> Result<Vec<u8>, LxcError> {
    let cmd_line = format!("{} {}", cli.binary(), args.join(" "));
    let fut = Command::new(cli.binary())
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let output = match timeout(Duration::from_secs(timeout_secs.max(1)), fut).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Err(LxcError::Command {
                cmd: cmd_line,
                stderr: e.to_string(),
            });
        }
        Err(_) => return Err(LxcError::Timeout(cmd_line)),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let lower = stderr.to_ascii_lowercase();
        if lower.contains("permission denied") {
            return Err(LxcError::PermissionDenied(stderr));
        }
        if lower.contains("connect: no such file or directory")
            || lower.contains("failed to connect")
            || lower.contains("unable to connect")
            || lower.contains("connection refused")
        {
            return Err(LxcError::DaemonUnreachable(stderr));
        }
        return Err(LxcError::Command {
            cmd: cmd_line,
            stderr,
        });
    }

    Ok(output.stdout)
}

/// Igual que `run`, pero manda `stdin_data` por stdin antes de esperar la
/// salida — lo único que necesita hoy es `profile edit`, que sin tty lee el
/// YAML nuevo de ahí en vez de abrir un editor.
pub async fn run_with_stdin(
    cli: Cli,
    args: &[&str],
    stdin_data: &[u8],
    timeout_secs: u64,
) -> Result<Vec<u8>, LxcError> {
    let cmd_line = format!("{} {}", cli.binary(), args.join(" "));
    let stdin_data = stdin_data.to_vec();

    let fut = async {
        let mut child = Command::new(cli.binary())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&stdin_data).await?;
        }

        child.wait_with_output().await
    };

    let output = match timeout(Duration::from_secs(timeout_secs.max(1)), fut).await {
        Ok(Ok(output)) => output,
        Ok(Err(e)) => {
            return Err(LxcError::Command {
                cmd: cmd_line,
                stderr: e.to_string(),
            });
        }
        Err(_) => return Err(LxcError::Timeout(cmd_line)),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(LxcError::Command { cmd: cmd_line, stderr });
    }

    Ok(output.stdout)
}

/// Corre un comando y parsea su stdout como JSON.
pub async fn run_json<T: serde::de::DeserializeOwned>(
    cli: Cli,
    args: &[&str],
    timeout_secs: u64,
) -> Result<T, LxcError> {
    let cmd_line = format!("{} {}", cli.binary(), args.join(" "));
    let raw = run(cli, args, timeout_secs).await?;
    serde_json::from_slice(&raw).map_err(|e| LxcError::Parse {
        cmd: cmd_line,
        detail: e.to_string(),
    })
}

/// Corre un comando cuyo único resultado relevante es el éxito/fracaso.
pub async fn run_ok(cli: Cli, args: &[&str], timeout_secs: u64) -> Result<(), LxcError> {
    run(cli, args, timeout_secs).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `lxc admin init` no existe: LXD se inicializa con el binario `lxd`.
    #[test]
    fn init_usa_el_binario_del_daemon_segun_el_backend() {
        assert_eq!(Cli::Incus.init_args(), ["incus", "admin", "init", "--auto"]);
        assert_eq!(Cli::Lxc.init_args(), ["lxd", "init", "--auto"]);
    }
}
