/**
 * Market data source — browser port of @dsh-market/core's data.js.
 *
 * The Node original fetched cfg.remoteUrl, wrote a disk cache, and fell back
 * remote → local file → stale cache. In the Tauri webview we have no fs, so:
 *   1. remote fetch (dsh.market serves CORS `*`, cache-control max-age=600)
 *   2. in-memory module cache (instant re-open within a session; also serves
 *      as the stale fallback if a later refresh fails)
 * The 14 MB payload is parsed once and kept in the module scope.
 */
import type { MarketData } from "./types";

/** Algorithm-native feed core is built against (pre-scored, ~5.8k plugins). */
export const REMOTE_URL = "https://dsh.market/plugins.json";

/** Treat an in-memory copy older than this as worth refreshing (matches the
 *  feed's max-age=600s). */
const FRESH_TTL_MS = 10 * 60 * 1000;

interface Cached {
  data: MarketData;
  fetchedAt: number;
}

let cache: Cached | null = null;
let inflight: Promise<MarketData> | null = null;

export interface MarketFetch {
  data: MarketData;
  source: "remote" | "cache";
  /** true when the network refresh failed and we served an older copy. */
  stale: boolean;
}

async function fetchRemote(): Promise<MarketData> {
  const res = await fetch(REMOTE_URL, {
    signal: AbortSignal.timeout(20000),
  });
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const data = (await res.json()) as MarketData;
  if (!Array.isArray(data?.plugins)) throw new Error("malformed market data");
  return data;
}

/**
 * Get market data. Serves a fresh in-memory copy when available; otherwise
 * fetches remote. On a network failure with a prior copy in hand, returns that
 * copy marked stale rather than throwing.
 */
export async function getMarketData(force = false): Promise<MarketFetch> {
  const now = Date.now();
  if (!force && cache && now - cache.fetchedAt < FRESH_TTL_MS) {
    return { data: cache.data, source: "cache", stale: false };
  }
  if (inflight) {
    const data = await inflight;
    return { data, source: "remote", stale: false };
  }
  inflight = fetchRemote();
  try {
    const data = await inflight;
    cache = { data, fetchedAt: now };
    return { data, source: "remote", stale: false };
  } catch (err) {
    if (cache) return { data: cache.data, source: "cache", stale: true };
    throw err;
  } finally {
    inflight = null;
  }
}
