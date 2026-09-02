use std::process::Command;

const SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const KEYBINDING_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/printcher/";
const NAME: &str = "printcher";
const DEFAULT_BINDING: &str = "<Control><Super>p";

/// Registra o printcher como atalho de teclado global do GNOME (via
/// `gsettings`), apontando para o binário atual. Idempotente: rodar de
/// novo só atualiza o comando/atalho em vez de duplicar a entrada.
pub fn install(binding: Option<String>) -> anyhow::Result<()> {
    let binding = binding.unwrap_or_else(|| DEFAULT_BINDING.to_string());
    let exe = std::env::current_exe()?;
    let exe = exe.to_string_lossy();

    let mut paths = parse_path_array(&gsettings_get(SCHEMA, "custom-keybindings")?);
    if !paths.iter().any(|p| p == PATH) {
        paths.push(PATH.to_string());
        gsettings_set(SCHEMA, "custom-keybindings", &format_path_array(&paths))?;
    }

    let relocatable = format!("{KEYBINDING_SCHEMA}:{PATH}");
    gsettings_set(&relocatable, "name", &quote(NAME))?;
    gsettings_set(&relocatable, "command", &quote(&exe))?;
    gsettings_set(&relocatable, "binding", &quote(&binding))?;

    println!("Atalho global configurado: {binding} -> {exe}");
    Ok(())
}

/// Remove o atalho global registrado por [`install`], se existir.
pub fn uninstall() -> anyhow::Result<()> {
    let mut paths = parse_path_array(&gsettings_get(SCHEMA, "custom-keybindings")?);
    let before = paths.len();
    paths.retain(|p| p != PATH);

    if paths.len() == before {
        println!("Nenhum atalho do printcher encontrado.");
        return Ok(());
    }

    gsettings_set(SCHEMA, "custom-keybindings", &format_path_array(&paths))?;
    println!("Atalho global removido.");
    Ok(())
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "\\'"))
}

fn gsettings_get(schema: &str, key: &str) -> anyhow::Result<String> {
    let output = Command::new("gsettings").args(["get", schema, key]).output()?;
    if !output.status.success() {
        anyhow::bail!(
            "gsettings get {schema} {key} falhou: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn gsettings_set(schema: &str, key: &str, value: &str) -> anyhow::Result<()> {
    let status = Command::new("gsettings").args(["set", schema, key, value]).status()?;
    if !status.success() {
        anyhow::bail!("gsettings set {schema} {key} {value} falhou");
    }
    Ok(())
}

/// Faz o parse de um array `as` no formato de saída do `gsettings get`,
/// por exemplo `['/a/', '/b/']` ou `@as []`.
fn parse_path_array(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    let raw = raw.strip_prefix("@as").map(str::trim).unwrap_or(raw);
    let inner = raw.trim_start_matches('[').trim_end_matches(']').trim();
    if inner.is_empty() {
        return Vec::new();
    }
    inner
        .split(',')
        .map(|s| s.trim().trim_matches('\'').to_string())
        .collect()
}

fn format_path_array(paths: &[String]) -> String {
    let items: Vec<String> = paths.iter().map(|p| quote(p)).collect();
    format!("[{}]", items.join(", "))
}
