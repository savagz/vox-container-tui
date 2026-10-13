use std::collections::BTreeMap;

use super::{
    CatalogImage, ImageAlias, ImageInfo, Instance, InstanceSnapshot, InstanceStatus, NetworkInfo,
    ProfileInfo, ServerEnvironment, ServerInfo, StoragePool, VolumeEntry,
};

/// Modo fixtures: datos falsos en vez de correr `incus`/`lxc` de verdad. Se activa
/// a mano con `VOXCONTAINER_FIXTURES=1` (tests, demos) o solo automáticamente en
/// macOS cuando no hay binario — sin esto la TUI sería indesarrollable fuera de un
/// host Linux con LXD.
pub fn enabled() -> bool {
    if std::env::var("VOXCONTAINER_FIXTURES").as_deref() == Ok("1") {
        return true;
    }
    cfg!(target_os = "macos") && super::cli::detect().is_none()
}

pub fn instances() -> Vec<Instance> {
    vec![
        Instance {
            name: "voxpbx-01".into(),
            status: InstanceStatus::Running,
            kind: "container".into(),
            architecture: "x86_64".into(),
            created_at: "2026-08-01T10:00:00Z".into(),
            ipv4: vec!["10.10.10.11".into()],
            cpu_ns: 12_500_000_000,
            cpu_pct: Some(4.2),
            memory_bytes: 512 * 1024 * 1024,
            disk_bytes: 2 * 1024 * 1024 * 1024,
            net_rx_bytes: 40 * 1024 * 1024,
            net_tx_bytes: 12 * 1024 * 1024,
            limits_cpu: Some("2".into()),
            limits_memory: Some("1GiB".into()),
            snapshots: vec![InstanceSnapshot {
                name: "pre-upgrade".into(),
                created_at: "2026-08-20T09:00:00Z".into(),
                stateful: false,
            }],
            profiles: vec!["default".into()],
        },
        Instance {
            name: "build-runner".into(),
            status: InstanceStatus::Stopped,
            kind: "container".into(),
            architecture: "x86_64".into(),
            created_at: "2026-07-15T08:30:00Z".into(),
            ipv4: vec![],
            cpu_ns: 0,
            cpu_pct: None,
            memory_bytes: 0,
            disk_bytes: 900 * 1024 * 1024,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            limits_cpu: None,
            limits_memory: None,
            snapshots: vec![],
            profiles: vec!["default".into()],
        },
        Instance {
            name: "db-cache".into(),
            status: InstanceStatus::Frozen,
            kind: "container".into(),
            architecture: "aarch64".into(),
            created_at: "2026-08-28T12:00:00Z".into(),
            ipv4: vec!["10.10.10.15".into()],
            cpu_ns: 3_000_000_000,
            cpu_pct: Some(0.0),
            memory_bytes: 128 * 1024 * 1024,
            disk_bytes: 512 * 1024 * 1024,
            net_rx_bytes: 5 * 1024 * 1024,
            net_tx_bytes: 1024 * 1024,
            limits_cpu: Some("1".into()),
            limits_memory: Some("256MiB".into()),
            snapshots: vec![],
            profiles: vec!["default".into()],
        },
        Instance {
            name: "web-edge".into(),
            status: InstanceStatus::Error,
            kind: "container".into(),
            architecture: "x86_64".into(),
            created_at: "2026-09-01T18:00:00Z".into(),
            ipv4: vec![],
            cpu_ns: 0,
            cpu_pct: None,
            memory_bytes: 0,
            disk_bytes: 300 * 1024 * 1024,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            limits_cpu: None,
            limits_memory: None,
            snapshots: vec![],
            profiles: vec!["default".into()],
        },
    ]
}

pub fn server_info() -> ServerInfo {
    ServerInfo {
        api_version: "1.0".into(),
        environment: ServerEnvironment {
            server_version: "6.x (fixtures)".into(),
            kernel: "n/a — modo fixtures".into(),
            storage: "dir".into(),
        },
    }
}

pub fn networks() -> Vec<NetworkInfo> {
    vec![NetworkInfo {
        name: "incusbr0".into(),
        kind: "bridge".into(),
        managed: true,
        used_by: vec!["/1.0/instances/voxpbx-01".into(), "/1.0/instances/db-cache".into()],
        config: BTreeMap::from([("ipv4.address".to_string(), "10.10.10.1/24".to_string())]),
    }]
}

pub fn images() -> Vec<ImageInfo> {
    vec![
        ImageInfo {
            fingerprint: "a1b2c3d4e5f6".into(),
            aliases: vec![ImageAlias { name: "debian/12".into() }],
            public: false,
            size: 180 * 1024 * 1024,
            architecture: "x86_64".into(),
            uploaded_at: "2026-06-01T00:00:00Z".into(),
            properties: BTreeMap::from([("description".to_string(), "Debian bookworm amd64 (default)".to_string())]),
        },
        ImageInfo {
            fingerprint: "f6e5d4c3b2a1".into(),
            aliases: vec![ImageAlias { name: "ubuntu/24.04".into() }],
            public: false,
            size: 210 * 1024 * 1024,
            architecture: "x86_64".into(),
            uploaded_at: "2026-05-12T00:00:00Z".into(),
            properties: BTreeMap::from([("description".to_string(), "Ubuntu noble amd64 (default)".to_string())]),
        },
    ]
}

/// Catálogo falso de `DOWNLOAD_REMOTES` — permite probar el modal de descarga
/// en macOS/`VOXCONTAINER_FIXTURES=1` sin pegarle de verdad a
/// linuxcontainers.org. Reparte entradas entre remotes distintos para ejercitar
/// la columna REMOTE y la referencia `remote:alias` al descargar.
pub fn remote_images() -> Vec<CatalogImage> {
    vec![
        CatalogImage {
            remote: "images",
            image: ImageInfo {
                fingerprint: "aa11bb22cc33".into(),
                aliases: vec![ImageAlias { name: "ubuntu/24.04".into() }],
                public: true,
                size: 220 * 1024 * 1024,
                architecture: "x86_64".into(),
                uploaded_at: "2026-08-10T00:00:00Z".into(),
                properties: BTreeMap::from([(
                    "description".to_string(),
                    "Ubuntu noble amd64 (fixtures)".to_string(),
                )]),
            },
        },
        CatalogImage {
            remote: "images",
            image: ImageInfo {
                fingerprint: "112233445566".into(),
                aliases: vec![ImageAlias { name: "debian/12".into() }],
                public: true,
                size: 180 * 1024 * 1024,
                architecture: "x86_64".into(),
                uploaded_at: "2026-06-01T00:00:00Z".into(),
                properties: BTreeMap::from([(
                    "description".to_string(),
                    "Debian bookworm amd64 (fixtures)".to_string(),
                )]),
            },
        },
        CatalogImage {
            remote: "images",
            image: ImageInfo {
                fingerprint: "778899aabbcc".into(),
                aliases: vec![ImageAlias { name: "alpine/3.20".into() }],
                public: true,
                size: 8 * 1024 * 1024,
                architecture: "x86_64".into(),
                uploaded_at: "2026-07-15T00:00:00Z".into(),
                properties: BTreeMap::from([(
                    "description".to_string(),
                    "Alpine 3.20 amd64 (fixtures)".to_string(),
                )]),
            },
        },
        CatalogImage {
            remote: "ubuntu",
            image: ImageInfo {
                fingerprint: "dd44ee55ff66".into(),
                aliases: vec![ImageAlias { name: "22.04".into() }],
                public: true,
                size: 200 * 1024 * 1024,
                architecture: "x86_64".into(),
                uploaded_at: "2026-04-01T00:00:00Z".into(),
                properties: BTreeMap::from([(
                    "description".to_string(),
                    "Ubuntu jammy amd64 (fixtures)".to_string(),
                )]),
            },
        },
        CatalogImage {
            remote: "ubuntu-daily",
            image: ImageInfo {
                fingerprint: "99887766aabb".into(),
                aliases: vec![ImageAlias { name: "24.04".into() }],
                public: true,
                size: 225 * 1024 * 1024,
                architecture: "x86_64".into(),
                uploaded_at: "2026-09-09T00:00:00Z".into(),
                properties: BTreeMap::from([(
                    "description".to_string(),
                    "Ubuntu noble amd64 daily (fixtures)".to_string(),
                )]),
            },
        },
    ]
}

pub fn pools() -> Vec<StoragePool> {
    vec![StoragePool {
        name: "default".into(),
        driver: "dir".into(),
        used_by: vec!["voxpbx-01".into(), "build-runner".into(), "db-cache".into()],
    }]
}

pub fn volumes() -> Vec<VolumeEntry> {
    vec![
        VolumeEntry {
            pool: "default".into(),
            name: "shared-data".into(),
            kind: "custom".into(),
            content_type: "filesystem".into(),
            used_by: vec!["/1.0/instances/voxpbx-01".into()],
        },
        VolumeEntry {
            pool: "default".into(),
            name: "backups".into(),
            kind: "custom".into(),
            content_type: "filesystem".into(),
            used_by: vec![],
        },
    ]
}

pub fn profiles() -> Vec<ProfileInfo> {
    vec![
        ProfileInfo {
            name: "default".into(),
            description: "Perfil por defecto (fixtures)".into(),
            used_by: vec![
                "/1.0/instances/voxpbx-01".into(),
                "/1.0/instances/build-runner".into(),
                "/1.0/instances/db-cache".into(),
                "/1.0/instances/web-edge".into(),
            ],
        },
        ProfileInfo {
            name: "sin-uso".into(),
            description: "Perfil de ejemplo sin asignar".into(),
            used_by: vec![],
        },
    ]
}

pub fn storage_show(name: &str) -> String {
    format!(
        "-- modo fixtures --\nname: {name}\ndriver: dir\nsource: /var/lib/incus/storage-pools/{name}\nconfig: {{}}\ndescription: \"\"\n"
    )
}

pub fn storage_pool_info(name: &str) -> String {
    format!("-- modo fixtures --\nname: {name}\nspace used: 3.2GiB\nspace total: 50.0GiB\n")
}

pub fn instance_log(name: &str) -> String {
    format!(
        "-- modo fixtures --\n[    0.000000] {name}: boot simulado, no hay backend real\n[    0.412000] {name}: systemd[1]: Reached target Multi-User System.\n"
    )
}

pub fn instance_info(name: &str) -> String {
    format!(
        "-- modo fixtures --\nName: {name}\nStatus: RUNNING\nType: container\nArchitecture: x86_64\nPID: 12345\nCreated: 2026-08-01T10:00:00Z\nLast Used: 2026-09-11T00:00:00Z\n\nResources:\n  Processes: 7\n  CPU usage: 4.2%\n  Memory usage: 512.0MiB\n  Network usage:\n    eth0: RX 40.0MiB, TX 12.0MiB\n"
    )
}

pub fn profile_show(name: &str) -> String {
    format!(
        "-- modo fixtures --\nname: {name}\ndescription: \"Perfil por defecto (fixtures)\"\nconfig:\n  limits.cpu: \"1\"\n  limits.memory: 512MiB\ndevices:\n  eth0:\n    name: eth0\n    network: incusbr0\n    type: nic\n  root:\n    path: /\n    pool: default\n    type: disk\nused_by:\n- /1.0/instances/voxpbx-01\n"
    )
}

pub fn instance_config_show(name: &str) -> String {
    format!(
        "-- modo fixtures --\narchitecture: x86_64\nconfig:\n  image.description: Debian bookworm amd64 (fixtures)\n  limits.cpu: \"2\"\n  limits.memory: 1GiB\n  volatile.base_image: a1b2c3d4e5f6\n  volatile.{name}.hwaddr: 00:16:3e:aa:bb:cc\ndevices:\n  root:\n    path: /\n    pool: default\n    type: disk\nephemeral: false\nprofiles:\n- default\nstateful: false\ndescription: \"\"\n"
    )
}

pub fn network_show(name: &str) -> String {
    format!(
        "-- modo fixtures --\nname: {name}\ntype: bridge\nconfig:\n  ipv4.address: 10.50.0.1/24\n  ipv4.nat: \"true\"\n  ipv6.address: none\nmanaged: true\nstatus: Created\n"
    )
}

pub fn network_leases(_name: &str) -> String {
    "HOSTNAME     MAC ADDRESS        IP ADDRESS  TYPE     \nvoxpbx-01    00:16:3e:aa:bb:cc  10.50.0.12  DYNAMIC\nbuild-runner 00:16:3e:dd:ee:ff  10.50.0.13  DYNAMIC\n".to_string()
}

pub fn image_info(name: &str) -> String {
    format!(
        "-- modo fixtures --\nFingerprint: aa11bb22cc33dd44\nSize: 220.00MiB\nArchitecture: x86_64\nType: container\nPublic: no\nTimestamps:\n    Created: 2026-08-10 00:00 UTC\nAliases:\n    - {name}\nProperties:\n    description: Imagen simulada\n"
    )
}

pub fn volume_show(volume: &str) -> String {
    format!(
        "-- modo fixtures --\nname: {volume}\ntype: custom\nconfig:\n  size: 10GiB\ndescription: \"\"\ncontent_type: filesystem\nused_by: []\n"
    )
}

pub fn operation_list() -> String {
    "-- modo fixtures --\nsin operaciones en curso\n".to_string()
}

pub fn warning_list() -> String {
    "-- modo fixtures --\nsin advertencias\n".to_string()
}
