/**
 * Tag utilities — port of @dsh-market/core's tags.js.
 * Filters out generic/low-signal tags and provides AND-matching helpers.
 */
import type { MarketPlugin } from "./types";

/** Broad tags that hit almost everything and carry no filtering value. */
const GENERIC_TAGS = new Set([
  "效率工具",
  "开发辅助",
  "AI 增强",
  "AI增强",
]);

const hasCJK = (t: string): boolean => /[一-鿿]/.test(t);

export interface TagCount {
  tag: string;
  count: number;
}

/** Aggregate all fine-grained Chinese tags (generic tags dropped). */
export function aggregateTags(plugins: MarketPlugin[]): TagCount[] {
  const counts = new Map<string, number>();
  for (const p of plugins) {
    for (const t of p.tags) {
      if (!hasCJK(t)) continue;
      if (GENERIC_TAGS.has(t)) continue;
      counts.set(t, (counts.get(t) ?? 0) + 1);
    }
  }
  return [...counts.entries()]
    .map(([tag, count]) => ({ tag, count }))
    .sort((a, b) => b.count - a.count);
}

/** Top N tags by frequency. */
export function hotTags(plugins: MarketPlugin[], n = 12): TagCount[] {
  return aggregateTags(plugins).slice(0, n);
}

/** AND semantics: plugin must carry every selected tag. */
export function matchesTags(plugin: MarketPlugin, selected: string[]): boolean {
  if (selected.length === 0) return true;
  return selected.every((t) => plugin.tags.includes(t));
}

/** How many of the selected tags a plugin carries. */
export function tagMatchCount(plugin: MarketPlugin, selected: string[]): number {
  return selected.filter((t) => plugin.tags.includes(t)).length;
}

/** Recommendable tags: fine-grained Chinese tags only. */
export function usableTags(plugin: MarketPlugin): string[] {
  return plugin.tags.filter((t) => hasCJK(t) && !GENERIC_TAGS.has(t));
}
