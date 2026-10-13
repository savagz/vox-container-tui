use std::time::Duration;

use tokio::sync::{mpsc, watch};

use crate::config::Config;
use crate::event::AppEvent;

use super::{Backend, Inventory, fixtures, query};

#[derive(Clone, Copy)]
enum Resource {
    Instances,
    Inventory,
}

/// Arranca las dos tareas de refresco en background, cada una con su propio
/// intervalo (`poll.instances_secs` / `poll.inventory_secs`). Guardar en el panel
/// de configuración empuja los intervalos nuevos por `config_rx` — recarga en
/// caliente, sin reiniciar el proceso.
pub fn spawn(backend: Backend, config_rx: watch::Receiver<Config>, tx: mpsc::Sender<AppEvent>) {
    spawn_loop(backend, config_rx.clone(), tx.clone(), Resource::Instances);
    spawn_loop(backend, config_rx, tx, Resource::Inventory);
}

fn spawn_loop(
    backend: Backend,
    mut config_rx: watch::Receiver<Config>,
    tx: mpsc::Sender<AppEvent>,
    resource: Resource,
) {
    tokio::spawn(async move {
        loop {
            let cfg = config_rx.borrow().clone();
            let timeout_secs = cfg.ui.command_timeout_secs;
            let lang = cfg.ui.language;
            let interval_secs = match resource {
                Resource::Instances => cfg.poll.instances_secs,
                Resource::Inventory => cfg.poll.inventory_secs,
            };

            match resource {
                Resource::Instances => poll_instances(backend, lang, timeout_secs, &tx).await,
                Resource::Inventory => poll_inventory(backend, lang, timeout_secs, &tx).await,
            }

            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(interval_secs.max(1))) => {}
                _ = config_rx.changed() => {}
            }
        }
    });
}

/// Localiza el error para la UI y deja el detalle completo (sin truncar, en
/// español fijo vía `Display`) en el log de la app — es la única forma de leer
/// el mensaje entero si no entra en el banner del header.
fn report_error(e: &super::cli::LxcError, lang: crate::i18n::Lang) -> String {
    tracing::error!(error = %e, "lxc daemon error");
    e.localized(lang)
}

/// `pub(crate)` (no sólo de uso interno del loop): `main.rs` también la llama
/// después de una acción exitosa (`spawn_action`), para refrescar ya mismo en
/// vez de esperar al próximo tick de `poll.instances_secs`.
pub(crate) async fn poll_instances(
    backend: Backend,
    lang: crate::i18n::Lang,
    timeout_secs: u64,
    tx: &mpsc::Sender<AppEvent>,
) {
    if backend.fixtures {
        let _ = tx
            .send(AppEvent::InstancesUpdated(fixtures::instances()))
            .await;
        return;
    }
    let Some(cli) = backend.cli else {
        let _ = tx
            .send(AppEvent::LxcError(report_error(&super::cli::LxcError::NotInstalled, lang)))
            .await;
        return;
    };
    match query::list_instances(cli, timeout_secs).await {
        Ok(instances) => {
            let _ = tx.send(AppEvent::InstancesUpdated(instances)).await;
        }
        Err(e) => {
            let _ = tx.send(AppEvent::LxcError(report_error(&e, lang))).await;
        }
    }
}

/// `pub(crate)`: mismo motivo que `poll_instances`, `main.rs` la usa para
/// refrescar el inventario (pools/volúmenes/imágenes/perfiles/redes) al toque
/// después de una acción exitosa.
pub(crate) async fn poll_inventory(
    backend: Backend,
    lang: crate::i18n::Lang,
    timeout_secs: u64,
    tx: &mpsc::Sender<AppEvent>,
) {
    if backend.fixtures {
        let inventory = Inventory {
            server: Some(fixtures::server_info()),
            networks: fixtures::networks(),
            images: fixtures::images(),
            pools: fixtures::pools(),
            volumes: fixtures::volumes(),
            profiles: fixtures::profiles(),
        };
        let _ = tx.send(AppEvent::InventoryUpdated(inventory)).await;
        return;
    }
    let Some(cli) = backend.cli else {
        // Ya se reportó "no instalado" desde poll_instances; no hay que repetirlo.
        return;
    };
    let (inventory, error) = query::inventory(cli, timeout_secs).await;
    let _ = tx.send(AppEvent::InventoryUpdated(inventory)).await;
    if let Some(e) = error {
        let _ = tx.send(AppEvent::LxcError(report_error(&e, lang))).await;
    }
}
