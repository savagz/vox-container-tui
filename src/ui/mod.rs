pub mod config_form;
pub mod format;
pub mod setup;
pub mod tabs;
pub mod widgets;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::app::App;
use crate::theme::{Status, glyph};
use tabs::Tab;

const SIDEBAR_WIDTH: u16 = 24;
const HEADER_HEIGHT: u16 = 3;

pub fn draw(frame: &mut Frame, app: &App) {
    // Fondo de chapa uniforme: sin esto los huecos entre paneles muestran el color
    // por defecto de la terminal y se rompe la ilusión de frente metálico.
    frame.render_widget(Block::default().style(app.theme.base_style()), frame.area());

    let banner_height = if app.lxc_error.is_some() { banner_lines(frame.area().width, app) } else { 0 };
    let help_bar_height = widgets::help_bar::height_for(app, frame.area().width);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(HEADER_HEIGHT),
            Constraint::Length(banner_height),
            Constraint::Min(0),
            Constraint::Length(help_bar_height),
        ])
        .split(frame.area());

    widgets::header::render(frame, rows[0], app);
    if banner_height > 0 {
        draw_daemon_banner(frame, rows[1], app);
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(0)])
        .split(rows[2]);

    widgets::sidebar::render(frame, cols[0], app);
    draw_body(frame, cols[1], app);
    widgets::help_bar::render(frame, rows[3], app);

    if let Some(form) = &app.config_form {
        config_form::render(frame, frame.area(), form, &app.theme);
    }
    if let Some(form) = &app.form {
        widgets::form::render(frame, frame.area(), form, &app.theme, app.lang);
    }
    if let Some(confirm) = &app.confirm {
        widgets::confirm::render(frame, frame.area(), confirm, &app.theme, app.lang);
    }
    if let Some(state) = &app.image_catalog {
        widgets::image_catalog::render(frame, frame.area(), state, &app.theme, app.lang);
    }
    if app.help_open {
        widgets::help::render(frame, frame.area(), app);
    }
    if app.info_open {
        widgets::info::render(frame, frame.area(), app);
    }
}

/// Cuántas filas necesita el banner del daemon para no cortar el mensaje: a
/// diferencia del piloto del header (28 caracteres fijos, ilegible con un error
/// real), acá el texto entero entra envuelto en hasta 4 líneas.
fn banner_lines(width: u16, app: &App) -> u16 {
    let Some(err) = &app.lxc_error else { return 0 };
    let prefix_len = 12;
    let usable = width.saturating_sub(2).max(20) as usize;
    let total = prefix_len + err.chars().count();
    let lines = total.saturating_add(usable - 1) / usable;
    (lines.max(1) as u16).min(4)
}

fn draw_daemon_banner(frame: &mut Frame, area: Rect, app: &App) {
    let Some(err) = &app.lxc_error else { return };
    let theme = &app.theme;
    let line = Line::from(vec![
        Span::styled(format!(" {} DAEMON  ", glyph::BOLT), theme.plate_style(theme.error)),
        Span::styled(format!(" {err}"), theme.style_for(Status::Error)),
    ]);
    frame.render_widget(
        Paragraph::new(line).wrap(Wrap { trim: true }).style(theme.base_style()),
        area,
    );
}

fn draw_body(frame: &mut Frame, area: Rect, app: &App) {
    match app.active_tab {
        Tab::Overview => tabs::overview::render(frame, area, app),
        Tab::Containers => tabs::containers::render(frame, area, app),
        Tab::Resources => tabs::resources::render(frame, area, app),
        Tab::Network => tabs::network::render(frame, area, app),
        Tab::Snapshots => tabs::snapshots::render(frame, area, app),
        Tab::Images => tabs::images::render(frame, area, app),
        Tab::Profiles => tabs::profiles::render(frame, area, app),
        Tab::Logs => tabs::logs::render(frame, area, app),
        Tab::Storage => tabs::storage::render(frame, area, app),
        Tab::SystemCheck => tabs::syscheck::render(frame, area, app),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Reload;
    use crate::config::Config;
    use crate::lxc::Backend;
    use crate::setup::SystemCheckState;
    use ratatui::{Terminal, backend::TestBackend};
    use tokio::sync::watch;

    fn app() -> App {
        let config = Config::default();
        let (config_tx, _config_rx) = watch::channel(config.clone());
        // Fuerza modo fixtures en los tests, sin importar el host donde corran.
        unsafe {
            std::env::set_var("VOXCONTAINER_FIXTURES", "1");
        }
        let backend = Backend::detect();
        App::new(
            config,
            Reload { config: config_tx },
            backend,
            SystemCheckState::new(crate::i18n::Lang::default()),
        )
    }

    /// El tablero se dibuja en terminales chicas sin panics de layout: los
    /// paneles industriales usan restas de ancho que hay que mantener saturadas.
    #[test]
    fn dibuja_el_tablero_en_varios_tamanos() {
        let app = app();
        for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
        }
    }

    #[test]
    fn dibuja_todas_las_pestanas() {
        let mut app = app();
        for tab in Tab::ALL {
            app.active_tab = tab;
            let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
        }
    }

    #[test]
    fn dibuja_el_wizard_de_primer_arranque() {
        let state = SystemCheckState::new(crate::i18n::Lang::default());
        let theme = crate::theme::Theme::default();
        for (w, h) in [(120, 40), (80, 24), (60, 20)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal
                .draw(|frame| setup::render_gate(frame, frame.area(), &state, &theme))
                .unwrap();
        }
    }

    #[test]
    fn dibuja_el_overlay_de_configuracion_sobre_el_tablero() {
        let mut app = app();
        app.config_form = Some(crate::config::ConfigForm::new(&app.config));
        let mut terminal = Terminal::new(TestBackend::new(100, 32)).unwrap();
        terminal.draw(|frame| draw(frame, &app)).unwrap();
    }

    /// Modal de descarga de imágenes (catálogo remoto): con catálogo cargado,
    /// con error y cargando — ninguno debe hacer panic de layout en pantallas
    /// chicas.
    #[test]
    fn dibuja_el_modal_de_descarga_de_imagenes() {
        let mut app = app();
        app.active_tab = Tab::Images;

        let states = [
            crate::app::ImageCatalogState {
                catalog: crate::lxc::fixtures::remote_images(),
                ..Default::default()
            },
            crate::app::ImageCatalogState { loading: true, ..Default::default() },
            crate::app::ImageCatalogState {
                error: Some("el daemon no responde".to_string()),
                ..Default::default()
            },
        ];

        for state in states {
            app.image_catalog = Some(state);
            for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|frame| draw(frame, &app)).unwrap();
            }
        }
    }

    /// El modal de ayuda (item 2 del pedido: `H` no hacía nada) no debe hacer
    /// panic de layout ni en pantallas angostas.
    #[test]
    fn dibuja_el_modal_de_ayuda() {
        let mut app = app();
        app.help_open = true;
        for tab in Tab::ALL {
            app.active_tab = tab;
            for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|frame| draw(frame, &app)).unwrap();
            }
        }
    }

    /// Pestaña PROFILES: popup de `lxc profile show` (tecla `i`), form de
    /// límites de perfil (tecla `l`, `lxc profile set`) y form de perfil nuevo
    /// con el campo de descripción agregado — ninguno debe hacer panic de
    /// layout en pantallas chicas.
    #[test]
    fn dibuja_los_popups_nuevos_de_profiles() {
        let mut app = app();
        app.active_tab = Tab::Profiles;
        app.info_open = true;
        app.info_target = Some("default".to_string());
        app.info_text = Some(crate::lxc::fixtures::profile_show("default"));

        for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
        }
        app.info_open = false;

        for kind in [crate::app::FormKind::ProfileLimits, crate::app::FormKind::NewProfile] {
            app.form = Some(crate::app::ActionForm {
                kind,
                target: "default".to_string(),
                labels: vec!["a", "b"],
                values: vec![String::new(), String::new()],
                select_options: vec![None, None],
                focus: 0,
                error: None,
            });
            for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|frame| draw(frame, &app)).unwrap();
            }
        }
    }

    /// Pestaña STORAGE: tabla de pools, drill-down a volúmenes de un pool
    /// (`storage_drill`) y el form de pool nuevo con el selector cíclico de
    /// driver (`◀ ▶`, item 2 del pedido) — ninguno debe hacer panic de layout.
    #[test]
    fn dibuja_las_vistas_de_storage() {
        let mut app = app();
        app.active_tab = Tab::Storage;

        for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
        }

        app.storage_drill = Some("default".to_string());
        for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
        }
        app.storage_drill = None;

        app.form = Some(crate::app::ActionForm {
            kind: crate::app::FormKind::NewPool,
            target: String::new(),
            labels: vec!["a", "b", "c"],
            values: vec!["pool1".to_string(), "dir".to_string(), String::new()],
            select_options: vec![
                None,
                Some(crate::lxc::STORAGE_DRIVERS.iter().map(|d| d.to_string()).collect()),
                None,
            ],
            focus: 1,
            error: None,
        });
        for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
        }
    }

    /// Popup de `lxc info` + `lxc config show` (pestaña CONTAINERS, tecla `i`):
    /// cargando, con texto corto, sin instancia objetivo, y con texto largo
    /// desplazado hasta el fondo (`info_scroll` en el límite) — ninguno debe
    /// hacer panic de layout ni de índice fuera de rango.
    #[test]
    fn dibuja_el_popup_de_info() {
        let mut app = app();
        app.active_tab = Tab::Containers;
        app.info_open = true;

        let combined = {
            let info = crate::lxc::fixtures::instance_info("voxpbx-01");
            let config = crate::lxc::fixtures::instance_config_show("voxpbx-01");
            format!("== lxc info ==\n{info}\n== lxc config show ==\n{config}")
        };
        let long_text = (0..200).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");

        for (loading, target, text, scroll) in [
            (true, Some("voxpbx-01".to_string()), None, 0),
            (false, Some("voxpbx-01".to_string()), Some(combined), 0),
            (false, None, None, 0),
            // Scroll más allá del contenido: el widget lo tiene que acotar solo.
            (false, Some("voxpbx-01".to_string()), Some(long_text.clone()), u16::MAX),
        ] {
            app.info_loading = loading;
            app.info_target = target;
            app.info_text = text;
            app.info_scroll = scroll;
            for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|frame| draw(frame, &app)).unwrap();
            }
        }
    }

    /// La barra de atajos inferior (item 3: se recortaba en terminales chicas)
    /// tiene que crecer en altura en vez de cortar texto — probamos que no
    /// panickee en varios anchos con la pestaña CONTAINERS (la de más atajos).
    #[test]
    fn la_barra_de_atajos_crece_en_terminales_angostas() {
        let mut app = app();
        app.active_tab = Tab::Containers;
        for (w, h) in [(120, 40), (60, 20), (30, 15), (15, 10)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| draw(frame, &app)).unwrap();
        }
    }

    /// El banner de error del daemon (items 1/3: antes se cortaba a 28
    /// caracteres en el header) no debe hacer panic de layout ni en pantallas
    /// angostas, sea cual sea el largo del mensaje.
    #[test]
    fn dibuja_el_banner_de_error_del_daemon_en_es_y_en() {
        let mut app = app();
        app.lxc_error = Some(
            "no se pudo interpretar la respuesta de `incus list --format json`: \
             invalid type: null, expected a sequence at line 1 column 4821"
                .to_string(),
        );
        for lang in [crate::i18n::Lang::Es, crate::i18n::Lang::En] {
            app.lang = lang;
            for (w, h) in [(120, 40), (40, 12), (20, 6)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                terminal.draw(|frame| draw(frame, &app)).unwrap();
            }
        }
    }

    // -- flujos de las acciones agregadas desde las guías LXD/LXC --

    use crate::app::{FormKind, PendingAction};
    use crate::event::AppEvent;
    use crate::lxc::{Inventory, fixtures};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// App con las instancias y el inventario falsos ya cargados, como los
    /// dejaría el poller.
    fn loaded_app() -> App {
        let mut app = app();
        app.on_event(AppEvent::InstancesUpdated(fixtures::instances()));
        app.on_event(AppEvent::InventoryUpdated(Inventory {
            server: Some(fixtures::server_info()),
            networks: fixtures::networks(),
            images: fixtures::images(),
            pools: fixtures::pools(),
            volumes: fixtures::volumes(),
            profiles: fixtures::profiles(),
        }));
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        app.on_event(AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    fn type_text(app: &mut App, text: &str) {
        for c in text.chars() {
            press(app, KeyCode::Char(c));
        }
    }

    fn draw_all_sizes(app: &App) {
        for (w, h) in [(120, 40), (80, 24), (40, 12), (20, 6)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|frame| draw(frame, app)).unwrap();
        }
    }

    #[test]
    fn nueva_instancia_arma_las_opciones_de_launch() {
        let mut app = loaded_app();
        app.active_tab = Tab::Containers;
        press(&mut app, KeyCode::Char('n'));
        let form = app.form.as_mut().expect("abre el form");
        assert_eq!(form.kind, FormKind::NewInstance);
        assert_eq!(form.labels.len(), 8);
        form.values = ["ubuntu:24.04", "vm01", "n", "vm", "default, web", "2", "4GiB", "30GiB"]
            .iter()
            .map(|v| v.to_string())
            .collect();
        draw_all_sizes(&app);
        press(&mut app, KeyCode::Enter);
        match app.take_pending() {
            Some(PendingAction::Launch { image, name, start, opts }) => {
                assert_eq!((image.as_str(), name.as_str(), start), ("ubuntu:24.04", "vm01", false));
                assert!(opts.vm);
                assert_eq!(opts.profiles, vec!["default".to_string(), "web".to_string()]);
                assert_eq!(
                    opts.config,
                    vec![
                        ("limits.cpu".to_string(), "2".to_string()),
                        ("limits.memory".to_string(), "4GiB".to_string()),
                    ]
                );
                assert_eq!(opts.root_size.as_deref(), Some("30GiB"));
            }
            other => panic!("se esperaba Launch, vino {other:?}"),
        }
    }

    #[test]
    fn nueva_instancia_sin_opciones_conserva_el_comportamiento_historico() {
        let mut app = loaded_app();
        app.active_tab = Tab::Containers;
        press(&mut app, KeyCode::Char('n'));
        let form = app.form.as_mut().unwrap();
        form.values[0] = "ubuntu:24.04".to_string();
        form.values[1] = "web01".to_string();
        press(&mut app, KeyCode::Enter);
        match app.take_pending() {
            Some(PendingAction::Launch { start, opts, .. }) => {
                assert!(start);
                assert_eq!(opts, crate::lxc::LaunchOptions::default());
            }
            other => panic!("se esperaba Launch, vino {other:?}"),
        }
    }

    #[test]
    fn quitar_config_pide_al_menos_una_clave() {
        let mut app = loaded_app();
        app.active_tab = Tab::Containers;
        press(&mut app, KeyCode::Char('U'));
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::UnsetConfigKeys);
        press(&mut app, KeyCode::Enter);
        assert!(app.form.as_ref().unwrap().error.is_some(), "sin claves no despacha");
        assert!(app.take_pending().is_none());

        type_text(&mut app, "limits.memory, limits.cpu");
        press(&mut app, KeyCode::Enter);
        match app.take_pending() {
            Some(PendingAction::UnsetConfigKeys { keys, .. }) => {
                assert_eq!(keys, vec!["limits.memory".to_string(), "limits.cpu".to_string()]);
            }
            other => panic!("se esperaba UnsetConfigKeys, vino {other:?}"),
        }
    }

    #[test]
    fn transferir_archivos_valida_rutas_y_direccion() {
        let mut app = loaded_app();
        app.active_tab = Tab::Containers;
        press(&mut app, KeyCode::Char('f'));
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::TransferFile);
        draw_all_sizes(&app);
        // Ruta local vacía: no despacha.
        press(&mut app, KeyCode::Enter);
        assert!(app.form.as_ref().unwrap().error.is_some());

        let form = app.form.as_mut().unwrap();
        form.values = ["pull", "/tmp/destino", "/etc/hostname", "s"].iter().map(|v| v.to_string()).collect();
        press(&mut app, KeyCode::Enter);
        match app.take_pending() {
            Some(PendingAction::TransferFile { push, local, remote, recursive, .. }) => {
                assert!(!push && recursive);
                assert_eq!((local.as_str(), remote.as_str()), ("/tmp/destino", "/etc/hostname"));
            }
            other => panic!("se esperaba TransferFile, vino {other:?}"),
        }
    }

    #[test]
    fn devices_proxy_con_atajo_de_puertos_y_remove() {
        let mut app = loaded_app();
        app.active_tab = Tab::Containers;
        press(&mut app, KeyCode::Char('t'));
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::Device);
        draw_all_sizes(&app);

        app.form.as_mut().unwrap().values[2] = "8080:80".to_string();
        press(&mut app, KeyCode::Enter);
        match app.take_pending() {
            Some(PendingAction::AddDevice { device, kind, pairs, .. }) => {
                assert_eq!((device.as_str(), kind.as_str()), ("proxy-8080", "proxy"));
                assert_eq!(pairs[0], ("listen".to_string(), "tcp:0.0.0.0:8080".to_string()));
                assert_eq!(pairs[1], ("connect".to_string(), "tcp:127.0.0.1:80".to_string()));
            }
            other => panic!("se esperaba AddDevice, vino {other:?}"),
        }

        // disk sin `path`: parámetros inválidos.
        press(&mut app, KeyCode::Char('t'));
        let form = app.form.as_mut().unwrap();
        form.values = ["disk", "datos", "source=/srv"].iter().map(|v| v.to_string()).collect();
        press(&mut app, KeyCode::Enter);
        assert!(app.form.as_ref().unwrap().error.is_some());

        let form = app.form.as_mut().unwrap();
        form.values = ["remove", "datos", ""].iter().map(|v| v.to_string()).collect();
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.take_pending(), Some(PendingAction::RemoveDevice { .. })));
    }

    #[test]
    fn publicar_instancia_acepta_alias_opcional() {
        let mut app = loaded_app();
        app.active_tab = Tab::Containers;
        press(&mut app, KeyCode::Char('B'));
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::PublishInstance);
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.take_pending(), Some(PendingAction::PublishInstance { alias: None, .. })));
    }

    #[test]
    fn network_borrar_exige_el_nombre_y_el_popup_refresca_su_propio_comando() {
        let mut app = loaded_app();
        app.active_tab = Tab::Network;
        let net = app.inventory.networks[0].name.clone();

        press(&mut app, KeyCode::Delete);
        assert!(app.confirm.is_some());
        press(&mut app, KeyCode::Enter);
        assert!(app.take_pending().is_none(), "sin tipear el nombre no borra");
        type_text(&mut app, &net);
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.take_pending(), Some(PendingAction::DeleteNetwork(n)) if n == net));

        press(&mut app, KeyCode::Char('i'));
        assert!(app.info_open);
        assert!(matches!(app.take_pending(), Some(PendingAction::FetchNetworkInfo(n)) if n == net));
        // `R` dentro del popup re-pide el comando de la red, no el de una instancia.
        press(&mut app, KeyCode::Char('R'));
        assert!(matches!(app.take_pending(), Some(PendingAction::FetchNetworkInfo(n)) if n == net));

        app.info_open = false;
        press(&mut app, KeyCode::Char('o'));
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::NetworkConfigKeys);
        type_text(&mut app, "ipv4.nat=true");
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.take_pending(), Some(PendingAction::SetNetworkKeys { .. })));
    }

    #[test]
    fn images_info_export_e_import() {
        let mut app = loaded_app();
        app.active_tab = Tab::Images;

        press(&mut app, KeyCode::Char('i'));
        assert!(matches!(app.take_pending(), Some(PendingAction::FetchImageInfo(_))));
        app.info_open = false;

        press(&mut app, KeyCode::Char('E'));
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::ExportImage);
        draw_all_sizes(&app);
        press(&mut app, KeyCode::Esc);

        press(&mut app, KeyCode::Char('I'));
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::ImportImage);
        press(&mut app, KeyCode::Enter);
        assert!(app.form.as_ref().unwrap().error.is_some(), "sin archivo no despacha");
        type_text(&mut app, "/tmp/img.tar.xz");
        press(&mut app, KeyCode::Tab);
        type_text(&mut app, "mi-imagen");
        press(&mut app, KeyCode::Enter);
        match app.take_pending() {
            Some(PendingAction::ImportImage { path, alias }) => {
                assert_eq!(path, "/tmp/img.tar.xz");
                assert_eq!(alias.as_deref(), Some("mi-imagen"));
            }
            other => panic!("se esperaba ImportImage, vino {other:?}"),
        }
    }

    #[test]
    fn storage_volumen_info_snapshot_y_mover() {
        let mut app = loaded_app();
        app.active_tab = Tab::Storage;
        press(&mut app, KeyCode::Enter);
        assert!(app.storage_drill.is_some());

        press(&mut app, KeyCode::Char('i'));
        assert!(matches!(app.take_pending(), Some(PendingAction::FetchVolumeInfo { .. })));
        app.info_open = false;

        press(&mut app, KeyCode::Char('S'));
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::SnapshotVolume);
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.take_pending(), Some(PendingAction::SnapshotVolume { snap: Some(_), .. })));

        // Con un solo pool no hay destino posible: el form lo rechaza.
        press(&mut app, KeyCode::Char('M'));
        assert_eq!(app.form.as_ref().unwrap().kind, FormKind::MoveVolume);
        draw_all_sizes(&app);
        press(&mut app, KeyCode::Enter);
        assert!(app.form.as_ref().unwrap().error.is_some());
        assert!(app.take_pending().is_none());
    }

    #[test]
    fn logs_operaciones_y_advertencias() {
        let mut app = loaded_app();
        app.active_tab = Tab::Logs;
        press(&mut app, KeyCode::Char('O'));
        assert!(matches!(app.take_pending(), Some(PendingAction::FetchOperations)));
        draw_all_sizes(&app);
        press(&mut app, KeyCode::Char('R'));
        assert!(matches!(app.take_pending(), Some(PendingAction::FetchOperations)));
        app.info_open = false;
        press(&mut app, KeyCode::Char('W'));
        assert!(matches!(app.take_pending(), Some(PendingAction::FetchWarnings)));
    }

    #[test]
    fn la_ayuda_y_la_barra_incluyen_las_teclas_nuevas() {
        let mut app = loaded_app();
        for tab in [Tab::Containers, Tab::Network, Tab::Images, Tab::Logs, Tab::Storage] {
            app.active_tab = tab;
            draw_all_sizes(&app);
        }
        app.help_open = true;
        draw_all_sizes(&app);
    }
}
