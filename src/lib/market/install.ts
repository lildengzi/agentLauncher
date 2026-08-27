/**
 * Install wiring — maps a MarketPlugin onto the launcher's existing Rust
 * commands (plugin_add / plugin_remove, both thin `dsh plugin` = pnpm wrappers).
 *
 * cordis-plugin  → pnpm add <spec> into the target profile (fully supported).
 * skill          → `git clone` into ~/.agents/skills; the launcher has no
 *                  Rust path for that yet, so skills are discovery-only and we
 *                  surface the exact command to run manually.
 */
import type { MarketPlugin } from "./types";

/** True when this plugin can be installed through plugin_add (pnpm). */
export function isInstallable(p: MarketPlugin): boolean {
  return p.type === "cordis-plugin";
}

/**
 * The package spec to hand to `dsh plugin add` / pnpm. Prefer the exact spec
 * from the feed's install command (e.g. `github:owner/repo` or an npm name);
 * fall back to `github:<fullName>` which pnpm resolves the same way.
 */
export function installSpec(p: MarketPlugin): string {
  const cmd = p.install.commands?.[0];
  if (cmd) {
    const m = cmd.match(/\badd\s+(\S+)/);
    if (m) return m[1];
  }
  return `github:${p.fullName}`;
}

/** The manual command for skill-type plugins (git clone into skills dir). */
export function skillInstallCommand(p: MarketPlugin): string {
  const dir = p.install.target || "~/.agents/skills";
  return `git clone --depth 1 https://github.com/${p.fullName}.git ${dir}/${p.name}`;
}

/**
 * Best-effort installed match against a profile's dependency names.
 * A pnpm install of `github:owner/repo` lands under the package's real name,
 * so we match by name / repo / spec tail rather than a single canonical key.
 */
export function matchedDep(
  p: MarketPlugin,
  installedPkgs: string[]
): string | null {
  if (installedPkgs.length === 0) return null;
  const spec = installSpec(p);
  const tail = spec.replace(/^github:/, "").split("/").pop() ?? "";
  const candidates = new Set(
    [p.name, p.repo, tail, spec, p.fullName]
      .filter(Boolean)
      .map((s) => s.toLowerCase())
  );
  return (
    installedPkgs.find((dep) => candidates.has(dep.toLowerCase())) ?? null
  );
}

export function isInstalled(p: MarketPlugin, installedPkgs: string[]): boolean {
  return matchedDep(p, installedPkgs) !== null;
}
