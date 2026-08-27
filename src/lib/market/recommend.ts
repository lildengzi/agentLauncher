/**
 * Recommend — port of @dsh-market/core's recommend.js.
 *
 * Ordering: scene (session-context tag hits) → guess (profile similarity, or
 * novice quality picks when no profile) → curated / trending fallback.
 *
 * The launcher has no GitHub binding, so `profile` is optional: without tag
 * weights we always take the novice branch (quality + no-config + curated),
 * which needs no user signals. When a lightweight profile is supplied (e.g.
 * derived from installed plugins), the veteran branch adds tag-similarity +
 * novelty + MMR diversity, exactly as upstream.
 */
import { usableTags } from "./tags";
import type { MarketPlugin } from "./types";

export const MMR_LAMBDA = 0.7;
export const NOVEL_DAYS = 30;

export interface UserProfile {
  /** tag → weight */
  tags: Record<string, number>;
}

export type Origin = "scene" | "guess" | "curated" | "trending";

export interface Recommendation {
  plugin: MarketPlugin;
  score: number;
  relevance: number;
  reasons: string[];
  origin: Origin;
}

export interface RecommendOptions {
  now?: Date;
  excludeIds?: string[];
  sceneTags?: string[];
  includeScene?: boolean;
  limit?: number;
}

export function recommend(
  plugins: MarketPlugin[],
  profile: UserProfile | null = null,
  options: RecommendOptions = {}
): Recommendation[] {
  const now = options.now ?? new Date();
  const exclude = new Set(options.excludeIds ?? []);
  const pool = plugins.filter((p) => !exclude.has(p.id));
  const limit = options.limit ?? 20;
  const out: Recommendation[] = [];

  // 1. scene: current-session tag hits
  if (options.includeScene !== false && options.sceneTags?.length) {
    const scenes = findSceneMatches(pool, options.sceneTags, now);
    for (const s of scenes.slice(0, 3)) {
      out.push({
        plugin: s.plugin,
        score: 1000 + s.hitTags.length * 10,
        relevance: Math.min(1, s.hitTags.length / options.sceneTags.length),
        reasons: [
          `当前场景涉及「${options.sceneTags.slice(0, 2).join("」「")}」`,
          ...genericReasons(s.plugin, now),
        ],
        origin: "scene",
      });
    }
  }

  // 2. guess: profile similarity, or novice quality picks
  const guessed =
    profile && Object.keys(profile.tags).length > 0
      ? veteranGuess(pool, profile, now)
      : noviceGuess(pool, now);
  for (const g of guessed) {
    if (out.some((o) => o.plugin.id === g.plugin.id)) continue;
    out.push(g);
  }

  // 3. fallback: curated
  for (const p of pool) {
    if (out.length >= limit) break;
    if (out.some((o) => o.plugin.id === p.id)) continue;
    if (p.curated) {
      out.push({
        plugin: p,
        score: 500 + p.score.total,
        relevance: 0,
        reasons: [p.curatedReason ?? "社区精选推荐"],
        origin: "curated",
      });
    }
  }

  // 4. fallback: newest active
  if (out.length < limit) {
    const newest = [...pool]
      .filter((p) => !out.some((o) => o.plugin.id === p.id))
      .sort(
        (a, b) =>
          new Date(b.pushedAt).getTime() - new Date(a.pushedAt).getTime()
      );
    for (const p of newest) {
      if (out.length >= limit) break;
      out.push({
        plugin: p,
        score: 100 + p.score.total,
        relevance: 0,
        reasons: ["最近更新活跃"],
        origin: "trending",
      });
    }
  }

  return out.sort((a, b) => b.score - a.score);
}

interface SceneMatch {
  plugin: MarketPlugin;
  hitTags: string[];
}

export function findSceneMatches(
  pool: MarketPlugin[],
  sceneTags: string[],
  _now: Date
): SceneMatch[] {
  return pool
    .map((p) => ({
      plugin: p,
      hitTags: sceneTags.filter((t) => looseTagHit(p, t)),
    }))
    .filter((m) => m.hitTags.length > 0)
    .sort(
      (a, b) =>
        b.hitTags.length - a.hitTags.length ||
        b.plugin.score.total - a.plugin.score.total
    );
}

/** Loose tag hit: exact/substring tag match + Chinese-summary substring. */
export function looseTagHit(p: MarketPlugin, tag: string): boolean {
  if (p.tags.includes(tag)) return true;
  if (p.tags.some((t) => t.includes(tag) || tag.includes(t))) return true;
  const zh = p.descriptionZh ?? "";
  return zh.length > 0 && zh.includes(tag);
}

/** Novice: high score + no-config-friendly + curated. Needs no profile. */
function noviceGuess(pool: MarketPlugin[], now: Date): Recommendation[] {
  const scored = pool.map((p) => {
    let s = p.score.total;
    if (!p.install.needsConfig) s += 8;
    if (p.curated) s += 10;
    if (p.score.confidence > 0.5) s += 3;
    return { p, s };
  });
  scored.sort((a, b) => b.s - a.s);
  return scored.slice(0, 10).map(({ p, s }) => ({
    plugin: p,
    score: s,
    relevance: 0,
    reasons: genericReasons(p, now),
    origin: "guess" as const,
  }));
}

/** Veteran: tag-weighted cosine similarity + novelty + MMR diversity. */
function veteranGuess(
  pool: MarketPlugin[],
  profile: UserProfile,
  now: Date
): Recommendation[] {
  const tagWeight = profile.tags;
  const profNorm = Object.values(tagWeight).reduce((a, b) => a + b * b, 0);

  const relevant = pool.map((p) => {
    const pTags = usableTags(p);
    let dot = 0;
    let hits = 0;
    for (const t of pTags) {
      const w = tagWeight[t] ?? 0;
      if (w > 0) {
        dot += w;
        hits += 1;
      }
    }
    const denom = Math.sqrt(hits) * Math.sqrt(profNorm);
    const similarity = denom === 0 ? 0 : Math.min(1, dot / denom);
    const novel =
      now.getTime() - new Date(p.pushedAt).getTime() <=
      NOVEL_DAYS * 86400000
        ? 0.2
        : 0;
    const scorePart = p.score.total / 100;
    return {
      p,
      similarity,
      novelty: novel,
      composite: similarity * 0.6 + scorePart * 0.2 + novel,
    };
  });

  const candidates = relevant
    .filter((r) => r.similarity > 0.02 || r.novelty > 0)
    .sort((a, b) => b.composite - a.composite);

  const selected: typeof candidates = [];
  const rest = [...candidates];
  while (rest.length && selected.length < 12) {
    let bestIdx = 0;
    let bestScore = -Infinity;
    for (let i = 0; i < rest.length; i++) {
      const c = rest[i];
      let maxSim = 0;
      for (const s of selected) {
        const sim = tagSimilarity(usableTags(c.p), usableTags(s.p));
        if (sim > maxSim) maxSim = sim;
      }
      const mmr = MMR_LAMBDA * c.composite - (1 - MMR_LAMBDA) * maxSim;
      if (mmr > bestScore) {
        bestScore = mmr;
        bestIdx = i;
      }
    }
    selected.push(rest[bestIdx]);
    rest.splice(bestIdx, 1);
  }

  return selected.map(({ p, similarity, novelty }) => ({
    plugin: p,
    score: 300 + similarity * 300 + novelty * 100,
    relevance: similarity,
    reasons: genericReasons(p, now),
    origin: "guess" as const,
  }));
}

/** Jaccard similarity of two tag sets. */
function tagSimilarity(a: string[], b: string[]): number {
  if (!a.length || !b.length) return 0;
  const setB = new Set(b);
  const inter = a.filter((t) => setB.has(t)).length;
  return inter / new Set([...a, ...b]).size;
}

/** Short time-based reason only (keeps cards compact, as upstream). */
function genericReasons(p: MarketPlugin, now: Date): string[] {
  const recent =
    now.getTime() - new Date(p.pushedAt).getTime() <= NOVEL_DAYS * 86400000;
  return recent ? ["近 30 天更新活跃"] : [];
}
