/**
 * Search — port of @dsh-market/core's search.js.
 * Fuse.js keyword recall + substring-priority boost + tag AND-filter + sort.
 */
import Fuse from "fuse.js";
import { matchesTags, usableTags } from "./tags";
import type { MarketPlugin, SearchOptions, SearchResult } from "./types";

/** Build the Fuse index (weights mirror the upstream tuning). */
export function createSearchIndex(plugins: MarketPlugin[]): Fuse<MarketPlugin> {
  return new Fuse(plugins, {
    keys: [
      { name: "name", weight: 0.4 },
      { name: "fullName", weight: 0.2 },
      { name: "descriptionZh", weight: 0.25 },
      { name: "description", weight: 0.1 },
      { name: "tags", weight: 0.05 },
    ],
    threshold: 0.4,
    ignoreLocation: true,
    includeScore: true,
    minMatchCharLength: 1,
  });
}

interface Hit {
  item: MarketPlugin;
  score: number;
}

/** Keyword search (empty query returns all, sorted by the chosen key). */
export function search(
  plugins: MarketPlugin[],
  query: string,
  options: SearchOptions = {}
): SearchResult[] {
  const q = query.trim();
  let hits: Hit[] = [];

  if (q) {
    const lower = q.toLowerCase();
    const seen = new Set<string>();
    // 1) direct substring hit on any field → absolute priority (score 0.05).
    //    Fuse normalizes by match-length ratio which penalizes long repo names;
    //    a plain substring hit should not depend on name length.
    for (const p of plugins) {
      const haystack = [
        p.name,
        p.fullName,
        p.descriptionZh ?? "",
        p.description,
        p.tags.join(" "),
      ]
        .join(" ")
        .toLowerCase();
      if (haystack.includes(lower)) {
        hits.push({ item: p, score: 0.05 });
        seen.add(p.id);
      }
    }
    // 2) fuzzy recall for the rest.
    const fuse = createSearchIndex(plugins);
    for (const r of fuse.search(q)) {
      if (seen.has(r.item.id)) continue;
      hits.push({ item: r.item, score: r.score ?? 1 });
    }
  } else {
    hits = plugins.map((p) => ({ item: p, score: 1 }));
  }

  const semantic = new Set(options.semanticTags ?? []);
  const tagFilter = options.tags ?? [];

  const results = hits
    .map((h) => {
      const p = h.item;
      const tagHits = [...semantic, ...tagFilter].filter((t) =>
        p.tags.includes(t)
      ).length;
      return {
        plugin: p,
        relevance: Math.round((1 - h.score) * 100),
        tagHits,
      };
    })
    .filter((r) => {
      const p = r.plugin;
      if (options.type && p.type !== options.type) return false;
      if (options.noConfigOnly && p.install.needsConfig) return false;
      if (!matchesTags(p, tagFilter)) return false;
      if (options.excludeIds?.includes(p.id)) return false;
      return true;
    });

  const sortBy = options.sortBy ?? "relevance";
  results.sort((a, b) => {
    if (sortBy === "score") return b.plugin.score.total - a.plugin.score.total;
    if (sortBy === "newest") {
      return (
        new Date(b.plugin.pushedAt).getTime() -
        new Date(a.plugin.pushedAt).getTime()
      );
    }
    // relevance: tag hits (strong semantic signal) → relevance → practical score
    return (
      b.tagHits - a.tagHits ||
      b.relevance - a.relevance ||
      b.plugin.score.total - a.plugin.score.total
    );
  });

  const sliced = options.limit ? results.slice(0, options.limit) : results;
  return sliced.map(({ plugin, relevance, tagHits }) => ({
    plugin,
    relevance,
    tagHits: tagHits + usableTags(plugin).filter((t) => semantic.has(t)).length,
  }));
}
