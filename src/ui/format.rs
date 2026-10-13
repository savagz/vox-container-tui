/// Formato legible de bytes para columnas de tabla: GiB si es grande, MiB si es
/// chico, `—` para cero (instancia detenida sin uso reportado).
pub fn bytes(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if bytes == 0 {
        "—".to_string()
    } else if b >= GIB {
        format!("{:.1}GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.0}MiB", b / MIB)
    } else {
        format!("{bytes}B")
    }
}
