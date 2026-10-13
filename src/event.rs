use crossterm::event::KeyEvent;

use crate::lxc::{CatalogImage, Instance, Inventory};
use crate::sysmon::snapshot::HostSnapshot;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Tick,
    Key(KeyEvent),
    /// Listado de instancias actualizado (alta frecuencia).
    InstancesUpdated(Vec<Instance>),
    /// Servidor + redes + imágenes + storage pools (baja frecuencia).
    InventoryUpdated(Inventory),
    /// Log de arranque pedido para una instancia puntual (bajo demanda, no poll).
    InstanceLog { name: String, log: String },
    /// `lxc info <name>` pedido para el popup de la pestaña CONTAINERS (bajo
    /// demanda, no poll).
    InstanceInfo { name: String, info: String },
    /// Catálogo del remote `images:` pedido bajo demanda para el modal de
    /// descarga de la pestaña IMAGES — nunca por el poller.
    RemoteImagesFetched(Result<Vec<CatalogImage>, String>),
    /// Falla de un comando `lxc`/`incus` durante el polling en background.
    LxcError(String),
    /// Resultado de una acción disparada por el usuario (start/stop/delete/...):
    /// mensaje para el toast y si fue éxito.
    ActionResult { message: String, ok: bool },
    SysSnapshot(HostSnapshot),
    /// SIGTERM/SIGINT recibido a nivel de proceso: hay que salir ya para restaurar la terminal.
    Shutdown,
}
