/**
 * dsh-market data model — mirrors the rich, pre-scored feed served at
 * https://dsh.market/plugins.json (schemaVersion 1). These are the exact
 * fields the ported search/tags/recommend algorithms consume; the five-
 * dimension PracticalScore is computed upstream by the collector, so the
 * frontend only reads it.
 */

export type PluginType = "cordis-plugin" | "skill";
export type InstallMethod = "pnpm-profile" | "skills-add";

export interface PracticalScoreBreakdown {
  maintain: number;
  practical: number;
  popularity: number;
  ease: number;
  signal: number;
}

export interface PracticalScore {
  /** 0-100 weighted total (used for sorting/recommend). */
  total: number;
  breakdown: PracticalScoreBreakdown;
  /** 0-1 signal richness. */
  confidence: number;
  /** Human-readable "why this score" sentence. */
  explanation: string;
}

export interface InstallInfo {
  method: InstallMethod;
  /** skill: target dir (~/.agents/skills). cordis: absent. */
  target?: string;
  /** Whether the plugin needs extra credentials/config to work. */
  needsConfig: boolean;
  /** cordis: the exact `dsh plugin --profile <p> add <spec>` command(s). */
  commands?: string[];
  commandSource?: string;
}

export interface MarketPlugin {
  /** owner/repo */
  id: string;
  type: PluginType;
  name: string;
  owner: string;
  repo: string;
  fullName: string;
  stars: number;
  forks: number;
  openIssues: number;
  language: string;
  description: string;
  descriptionZh: string;
  tags: string[];
  curated: boolean;
  curatedReason?: string;
  homepage: string;
  license: string;
  topics: string[];
  pushedAt: string;
  createdAt: string;
  updatedAt: string;
  readmeSummary: string;
  introByAuthor: string;
  submissionIssue?: number;
  install: InstallInfo;
  score: PracticalScore;
  sources: string[];
  lastCheckedAt: string;
}

export interface MarketData {
  schemaVersion: number;
  generatedAt: string;
  plugins: MarketPlugin[];
  packs: unknown[];
}

/** A single search result row. */
export interface SearchResult {
  plugin: MarketPlugin;
  /** 0-100 keyword relevance. */
  relevance: number;
  /** number of selected/semantic tags this plugin matched. */
  tagHits: number;
}

export interface SearchOptions {
  tags?: string[];
  semanticTags?: string[];
  type?: PluginType;
  noConfigOnly?: boolean;
  excludeIds?: string[];
  sortBy?: "relevance" | "score" | "newest";
  limit?: number;
}
