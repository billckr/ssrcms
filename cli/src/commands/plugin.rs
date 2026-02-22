use clap::Subcommand;
use std::path::Path;

#[derive(Subcommand)]
pub enum PluginAction {
    /// List installed plugins (reads plugin.toml manifests from ./plugins/)
    List,
}

pub async fn run(action: PluginAction) -> anyhow::Result<()> {
    match action {
        PluginAction::List => list(),
    }
}

fn list() -> anyhow::Result<()> {
    let plugins_dir = Path::new("plugins");

    if !plugins_dir.exists() {
        println!("No plugins directory found (expected: ./plugins/)");
        return Ok(());
    }

    let mut found = false;
    let entries = std::fs::read_dir(plugins_dir)
        .map_err(|e| anyhow::anyhow!("Cannot read plugins dir: {e}"))?;

    println!("\n{:<20} {:<10} {:<10} {}", "Name", "Version", "API", "Description");
    println!("{}", "-".repeat(70));

    for entry in entries.flatten() {
        let manifest_path = entry.path().join("plugin.toml");
        if !manifest_path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&manifest_path)
            .map_err(|e| anyhow::anyhow!("Cannot read {}: {e}", manifest_path.display()))?;

        // Parse just the fields we need from TOML without pulling in a full model
        let table: toml::Value = content.parse()
            .map_err(|e| anyhow::anyhow!("Invalid TOML in {}: {e}", manifest_path.display()))?;

        let plugin = table.get("plugin").unwrap_or(&table);
        let name        = plugin.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let version     = plugin.get("version").and_then(|v| v.as_str()).unwrap_or("?");
        let api_version = plugin.get("api_version").and_then(|v| v.as_str()).unwrap_or("?");
        let description = plugin.get("description").and_then(|v| v.as_str()).unwrap_or("");

        println!("{:<20} {:<10} {:<10} {}", name, version, api_version, description);
        found = true;
    }

    if !found {
        println!("(no plugins found)");
    }

    Ok(())
}
