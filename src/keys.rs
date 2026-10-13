use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    Quit,
    NextTab,
    PrevTab,
    /// Salto directo a una sección del menú (`1`..`9`).
    JumpTab(usize),
    Up,
    Down,
    Top,
    Bottom,
    /// Abre el panel de configuración (intervalos de polling, timeout).
    OpenConfig,
    /// Fuerza un refresco inmediato, sin esperar al intervalo de polling.
    Refresh,
    Help,
    /// Filtro por nombre/estado en la tabla de `CONTAINERS`.
    Filter,
    // Acciones de ciclo de vida sobre la instancia seleccionada (pestaña CONTAINERS):
    Start,
    Stop,
    Restart,
    PauseToggle,
    New,
    Clone,
    Snapshot,
    Console,
    Limits,
    Delete,
    /// Redefine el alias de la imagen seleccionada (pestaña IMAGES).
    EditAlias,
    /// Asigna la lista completa de perfiles a la instancia seleccionada
    /// (pestaña CONTAINERS).
    AssignProfiles,
    /// Edita la descripción del perfil seleccionado (pestaña PROFILES).
    EditDescription,
    /// Salta a la pestaña LOGS con la instancia seleccionada (pestaña CONTAINERS).
    ViewLogs,
    /// Abre el popup con `lxc info` de la instancia seleccionada (pestaña
    /// CONTAINERS).
    Info,
    /// Abre el catálogo remoto (`images:`) para descargar una imagen nueva
    /// (pestaña IMAGES).
    Download,
    /// Renombra/mueve la instancia seleccionada (`lxc move`) (pestaña
    /// CONTAINERS).
    Move,
    /// Abre el formulario de claves de config libres (`lxc config set
    /// <key>=<value>`), no sólo límites de CPU/memoria (pestaña CONTAINERS).
    ConfigKeys,
    /// Adjunta el volumen seleccionado a una instancia (`lxc storage volume
    /// attach`) (pestaña STORAGE).
    Attach,
    /// Desvincula el volumen seleccionado de una instancia (`lxc storage
    /// volume detach`) (pestaña STORAGE).
    Detach,
    /// Entra a los volúmenes del pool seleccionado (pestaña STORAGE). Sin uso
    /// fuera de ahí: todo modal que ya usa Enter (formularios, confirmaciones,
    /// catálogo de imágenes) lo consume en su propio handler antes de llegar
    /// al dispatch general.
    Enter,
    /// Vuelve de los volúmenes de un pool a la lista de pools (pestaña
    /// STORAGE). Mismo motivo que `Enter`: todo modal existente cierra con Esc
    /// en su propio handler, nunca llega acá.
    Back,
    /// Backup de la instancia seleccionada a un archivo (`lxc export`)
    /// (pestaña CONTAINERS).
    Export,
    /// Crea una instancia nueva desde un backup (`lxc import`) (pestaña
    /// CONTAINERS). No necesita instancia seleccionada.
    Import,
    /// Quita claves de config de la instancia (`lxc config unset`) (pestaña
    /// CONTAINERS).
    Unset,
    /// Sube/baja archivos entre el host y la instancia (`lxc file push|pull`)
    /// (pestaña CONTAINERS).
    Files,
    /// Agrega/quita devices de la instancia (`lxc config device`): puertos
    /// publicados (`proxy`), carpetas montadas (`disk`) (pestaña CONTAINERS).
    Devices,
    /// Publica la instancia como imagen local (`lxc publish`) (pestaña
    /// CONTAINERS).
    Publish,
    /// Mueve el volumen seleccionado a otro pool (`lxc storage volume move`)
    /// (pestaña STORAGE).
    MoveTo,
    /// `lxc operation list` (pestaña LOGS).
    Operations,
    /// `lxc warning list` (pestaña LOGS).
    Warnings,
}

pub fn action_for(key: KeyEvent) -> Option<AppAction> {
    match key.code {
        KeyCode::Char('q') => Some(AppAction::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(AppAction::Quit)
        }
        KeyCode::Char('c') => Some(AppAction::OpenConfig),
        KeyCode::F(1) => Some(AppAction::OpenConfig),
        KeyCode::Tab => Some(AppAction::NextTab),
        KeyCode::BackTab => Some(AppAction::PrevTab),
        KeyCode::Char(d @ '1'..='9') => {
            Some(AppAction::JumpTab(d.to_digit(10).unwrap() as usize - 1))
        }
        // Décima pestaña (STORAGE se sumó después de que `1`-`9` ya cubrían
        // las primeras nueve): `0` salta a la última, sin reordenar el resto.
        KeyCode::Char('0') => Some(AppAction::JumpTab(9)),
        KeyCode::Up | KeyCode::Char('k') => Some(AppAction::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(AppAction::Down),
        KeyCode::Home | KeyCode::Char('g') => Some(AppAction::Top),
        KeyCode::End | KeyCode::Char('G') => Some(AppAction::Bottom),
        KeyCode::Char('r') => Some(AppAction::Refresh),
        KeyCode::Char('h') | KeyCode::Char('H') => Some(AppAction::Help),
        KeyCode::Char('/') => Some(AppAction::Filter),
        KeyCode::Char('s') => Some(AppAction::Start),
        KeyCode::Char('x') => Some(AppAction::Stop),
        KeyCode::Char('R') => Some(AppAction::Restart),
        KeyCode::Char('p') => Some(AppAction::PauseToggle),
        KeyCode::Char('n') => Some(AppAction::New),
        KeyCode::Char('d') => Some(AppAction::Clone),
        KeyCode::Char('S') => Some(AppAction::Snapshot),
        KeyCode::Char('e') => Some(AppAction::Console),
        KeyCode::Char('l') => Some(AppAction::Limits),
        KeyCode::Delete | KeyCode::Backspace => Some(AppAction::Delete),
        KeyCode::Char('a') => Some(AppAction::EditAlias),
        KeyCode::Char('P') => Some(AppAction::AssignProfiles),
        KeyCode::Char('D') => Some(AppAction::EditDescription),
        KeyCode::Char('v') => Some(AppAction::ViewLogs),
        KeyCode::Char('i') => Some(AppAction::Info),
        KeyCode::Char('w') => Some(AppAction::Download),
        KeyCode::Char('m') => Some(AppAction::Move),
        KeyCode::Char('o') => Some(AppAction::ConfigKeys),
        KeyCode::Char('b') => Some(AppAction::Attach),
        KeyCode::Char('u') => Some(AppAction::Detach),
        KeyCode::Enter => Some(AppAction::Enter),
        KeyCode::Esc => Some(AppAction::Back),
        KeyCode::Char('E') => Some(AppAction::Export),
        KeyCode::Char('I') => Some(AppAction::Import),
        KeyCode::Char('U') => Some(AppAction::Unset),
        KeyCode::Char('f') => Some(AppAction::Files),
        KeyCode::Char('t') => Some(AppAction::Devices),
        KeyCode::Char('B') => Some(AppAction::Publish),
        KeyCode::Char('M') => Some(AppAction::MoveTo),
        KeyCode::Char('O') => Some(AppAction::Operations),
        KeyCode::Char('W') => Some(AppAction::Warnings),
        _ => None,
    }
}
