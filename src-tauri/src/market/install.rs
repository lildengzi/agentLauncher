//! Turning a chosen `InstallSpec` into the real side effect.
//!
//! Nothing here is a launcher-invented install mechanism: each method routes to
//! the place that kind of extension already lives (a dsh profile's pnpm deps, the
//! instance's `skills/` directory, the instance's `mcp.json`). The market only
//! decides *what*; these functions are the *where*, and they are the same paths the
//! edit dialog's own sections read back.

use std::path::PathBuf;

use super::InstallSpec;
use crate::instance_ext::McpServerEntry;
use crate::instance_manager;

/// Directory-safe form of a market item's name, for `skills/<dir>`.
fn slug(name: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "skill".into()
    } else {
        s
    }
}

fn skills_dir(instance_id: &str) -> Result<PathBuf, String> {
    Ok(instance_manager::instance_dir(instance_id)?.join("skills"))
}

/// `git clone --depth 1 <repo> skills/<slug>`.
///
/// Shelling out to git rather than vendoring a client: a skill repo is exactly what
/// its author publishes, and the user's own git config (credentials, proxies,
/// insteadOf rewrites) is the thing most likely to make a clone work.
async fn clone_skill(instance_id: &str, repo: &str, name: &str) -> Result<String, String> {
    if repo.trim().is_empty() {
        return Err("this item has no repository to clone".into());
    }
    let dir = skills_dir(instance_id)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let target = dir.join(slug(name));
    if target.exists() {
        return Err(format!(
            "{} already exists — remove it first",
            target.display()
        ));
    }
    let out = tokio::process::Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(repo.trim())
        .arg(&target)
        .output()
        .await
        .map_err(|e| format!("git clone failed to start: {e} (is git installed?)"))?;
    if !out.status.success() {
        // A half-written clone would show up as an installed-but-broken skill.
        std::fs::remove_dir_all(&target).ok();
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(target.to_string_lossy().to_string())
}

/// Merge one `mcpServers` entry into the instance, replacing a same-named one.
fn add_mcp(instance_id: &str, entry: &McpServerEntry) -> Result<String, String> {
    if entry.name.trim().is_empty() || entry.command.trim().is_empty() {
        return Err("this item's MCP definition has no name or command".into());
    }
    let mut servers = crate::instance_ext::read_mcp(instance_id)?;
    servers.retain(|s| s.name != entry.name);
    servers.push(entry.clone());
    crate::instance_ext::set_instance_mcp(instance_id.to_string(), servers)?;
    Ok(entry.name.clone())
}

/// Install one market item into one instance.
///
/// `name` is the item's display name, used only to derive a skill directory. The
/// returned string is a short human-readable note about what happened (a path, a
/// package, a server name) for the dialog's status line.
#[tauri::command]
pub async fn market_install(
    instance_id: String,
    name: String,
    spec: InstallSpec,
) -> Result<String, String> {
    let inst = instance_manager::get_instance(&instance_id)?;
    match spec.method.as_str() {
        "pnpm-profile" => {
            if spec.package.trim().is_empty() {
                return Err("this item has no package to install".into());
            }
            let profile = if inst.profile.is_empty() {
                "headless".to_string()
            } else {
                inst.profile.clone()
            };
            crate::runtime::dsh_home::plugin_add(profile, spec.package.trim().to_string()).await
        }
        "git-clone" => clone_skill(&instance_id, &spec.repo, &name).await,
        "mcp-config" => {
            let entry = spec
                .mcp
                .as_ref()
                .ok_or("this item has no MCP server definition")?;
            add_mcp(&instance_id, entry)
        }
        "manual" | "" => Err("this item installs manually — copy the command instead".into()),
        other => Err(format!("unknown install method: {other}")),
    }
}

/// Undo `market_install` for the same item.
#[tauri::command]
pub async fn market_uninstall(
    instance_id: String,
    name: String,
    spec: InstallSpec,
) -> Result<String, String> {
    let inst = instance_manager::get_instance(&instance_id)?;
    match spec.method.as_str() {
        "pnpm-profile" => {
            let profile = if inst.profile.is_empty() {
                "headless".to_string()
            } else {
                inst.profile.clone()
            };
            crate::runtime::dsh_home::plugin_remove(profile, spec.package.trim().to_string()).await
        }
        "git-clone" => {
            crate::instance_ext::remove_instance_skill(instance_id, slug(&name))?;
            Ok(slug(&name))
        }
        "mcp-config" => {
            let target = spec
                .mcp
                .as_ref()
                .map(|m| m.name.clone())
                .unwrap_or_else(|| name.clone());
            let mut servers = crate::instance_ext::read_mcp(&instance_id)?;
            let before = servers.len();
            servers.retain(|s| s.name != target);
            if servers.len() == before {
                return Err(format!("no MCP server named {target}"));
            }
            crate::instance_ext::set_instance_mcp(instance_id, servers)?;
            Ok(target)
        }
        other => Err(format!("cannot uninstall method: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_directory_safe() {
        assert_eq!(slug("PDF Forms"), "pdf-forms");
        assert_eq!(slug("@scope/pkg.name"), "scope-pkg-name");
        assert_eq!(slug("../.."), "skill");
        assert_eq!(slug(""), "skill");
    }
}
