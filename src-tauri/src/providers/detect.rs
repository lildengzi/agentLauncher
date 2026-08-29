//! Listing models — locally by port probe, remotely by asking the provider.
//!
//! Two very different acts, kept in one file because the UI presents them as one
//! question ("what can I actually run?"), and kept clearly apart in the code
//! because only one of them sends the user's key anywhere:
//!
//! * [`detect_local_llms`] probes loopback ports for Ollama / LM Studio / vLLM /
//!   llama.cpp. No credentials, no proxy, nothing leaves the machine.
//! * [`fetch_provider_models`] calls one provider's own `/models` endpoint with a
//!   stored key. This is the only outbound request in the launcher that carries a
//!   secret, and it happens **only when the user presses the button for that
//!   provider** — never on open, never on a timer, never for a provider they did not
//!   name. The key is read from disk here and never passes through the frontend.

use std::time::Duration;

use serde::Serialize;

use super::load;

/// Loopback only. A remote "local" runtime is not a thing this probe will reach for,
/// because a hostname here would turn a convenience button into an outbound scan.
const LOOPBACK: &str = "127.0.0.1";
const PROBE_TIMEOUT: Duration = Duration::from_millis(800);
/// A models list is a few KiB. The cap is what keeps a misidentified endpoint from
/// streaming a video into memory.
const MAX_BODY: usize = 2 * 1024 * 1024;

/// One local runtime that answered, with the models it reported.
#[derive(Serialize, Clone, Debug)]
pub struct LocalLlm {
    pub id: String,
    pub label: String,
    /// OpenAI-compatible root, ready to paste into a provider row.
    pub base_url: String,
    pub models: Vec<String>,
}

/// `(id, label, port, path)` — the four runtimes worth guessing at.
///
/// The last two ports are also every developer's default HTTP port, which is why a
/// hit requires the body to *parse as a models list*: a Vite dev server on :8080
/// answers 200 with HTML and is correctly ignored.
const LOCAL_RUNTIMES: &[(&str, &str, u16, &str)] = &[
    ("ollama", "Ollama", 11434, "/api/tags"),
    ("lmstudio", "LM Studio", 1234, "/v1/models"),
    ("vllm", "vLLM", 8000, "/v1/models"),
    ("llamacpp", "llama.cpp", 8080, "/v1/models"),
];

/// A client for loopback probing. `no_proxy` is the point: reqwest honours
/// `HTTP_PROXY` by default, and sending a localhost probe through a corporate proxy
/// is both broken and the opposite of "local only".
fn probe_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(PROBE_TIMEOUT)
        .timeout(PROBE_TIMEOUT)
        .build()
        .map_err(|e| format!("cannot build probe client: {e}"))
}

/// Pull model names out of either shape: OpenAI's `data[].id` or Ollama's
/// `models[].name`. Anything else yields an empty list, which is how a non-LLM
/// endpoint gets rejected.
fn model_names(body: &serde_json::Value) -> Vec<String> {
    let from = |arr: Option<&Vec<serde_json::Value>>, field: &str| -> Vec<String> {
        arr.map(|items| {
            items
                .iter()
                .filter_map(|i| i.get(field).and_then(|v| v.as_str()))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
    };
    let mut out = from(body.get("data").and_then(|d| d.as_array()), "id");
    if out.is_empty() {
        out = from(body.get("models").and_then(|d| d.as_array()), "name");
    }
    if out.is_empty() {
        out = from(body.get("models").and_then(|d| d.as_array()), "id");
    }
    out.sort();
    out.dedup();
    out
}

async fn read_json(resp: reqwest::Response) -> Result<serde_json::Value, String> {
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BODY {
        return Err("响应过大".into());
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

async fn probe_one(
    client: reqwest::Client,
    id: &'static str,
    label: &'static str,
    port: u16,
    path: &'static str,
) -> Option<LocalLlm> {
    let url = format!("http://{LOOPBACK}:{port}{path}");
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let models = model_names(&read_json(resp).await.ok()?);
    if models.is_empty() {
        return None;
    }
    Some(LocalLlm {
        id: id.to_string(),
        label: label.to_string(),
        // Every one of these serves an OpenAI-compatible `/v1`, Ollama included.
        base_url: format!("http://{LOOPBACK}:{port}/v1"),
        models,
    })
}

/// Probe every known local runtime concurrently and report the ones that answered
/// with something that looks like a models list. A runtime that is not running is
/// simply absent from the result — not an error, since "nothing found" is the
/// expected answer on most machines.
#[tauri::command]
pub async fn detect_local_llms() -> Result<Vec<LocalLlm>, String> {
    let client = probe_client()?;
    let tasks: Vec<_> = LOCAL_RUNTIMES
        .iter()
        .map(|(id, label, port, path)| {
            tokio::spawn(probe_one(client.clone(), id, label, *port, path))
        })
        .collect();
    let mut found = Vec::new();
    for t in tasks {
        if let Ok(Some(llm)) = t.await {
            found.push(llm);
        }
    }
    Ok(found)
}

// ---- remote: ask the provider ---------------------------------------------
// Most people's keys are a cloud provider's, so a local-only probe would leave the
// common case hand-typed. This is that missing half — with the blast radius kept
// small: one provider per press, its own base URL, https unless it is loopback, no
// redirects, and the key read from disk inside this process.

const FETCH_TIMEOUT: Duration = Duration::from_secs(20);

/// Where a provider's model list lives, derived from its API root.
///
/// If the root already ends in a version segment (`/v1`, `/v3`) the endpoint is
/// `<root>/models`; otherwise `/v1/models` is appended, which is what every
/// OpenAI-compatible service serves. A root the user typed with a trailing slash
/// works either way.
fn models_url(base: &str) -> Result<reqwest::Url, String> {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("这个 provider 还没有填 Base URL".into());
    }
    let url = reqwest::Url::parse(trimmed).map_err(|e| format!("Base URL 无法解析: {e}"))?;
    let host = url.host_str().unwrap_or_default();
    let loopback = matches!(host, "127.0.0.1" | "localhost" | "::1" | "[::1]");
    // A key must not be sent in the clear to anything but this machine.
    if url.scheme() != "https" && !loopback {
        return Err("拉取模型列表要求 https（本机地址除外）".into());
    }
    let versioned = url
        .path_segments()
        .and_then(|mut s| s.rfind(|s: &&str| !s.is_empty()))
        .map(|last| {
            let mut c = last.chars();
            c.next() == Some('v') && c.all(|ch| ch.is_ascii_digit())
        })
        .unwrap_or(false);
    let suffix = if versioned { "/models" } else { "/v1/models" };
    reqwest::Url::parse(&format!("{trimmed}{suffix}")).map_err(|e| e.to_string())
}

/// Ask one provider which models the given key can see.
///
/// `alias` pins which stored key to authenticate with; empty picks the provider's
/// first enabled key that has a value. The list is *returned*, not saved — it lands
/// in the settings form as an unsaved edit, so the user still decides what the
/// provider's model list becomes.
#[tauri::command]
pub async fn fetch_provider_models(provider: String, alias: String) -> Result<Vec<String>, String> {
    // Resolve URL, style and key up front so the file read is over before any socket
    // is opened, and so a misconfigured row fails without a request at all.
    let (url, auth_style, key) = {
        let doc = load()?;
        let p = doc
            .providers
            .iter()
            .find(|p| p.id == provider)
            .ok_or_else(|| format!("没有这个 provider: {provider}"))?;
        let alias = alias.trim();
        let chosen = if alias.is_empty() {
            p.keys
                .iter()
                .find(|k| k.enabled && !k.value.is_empty())
                .ok_or("这个 provider 下没有可用的 API Key")?
        } else {
            let k = p
                .keys
                .iter()
                .find(|k| k.alias == alias)
                .ok_or_else(|| format!("没有名为「{alias}」的 API Key"))?;
            if k.value.is_empty() {
                return Err(format!("API Key「{alias}」还没有填入值"));
            }
            k
        };
        (models_url(&p.base_url)?, p.auth_style.clone(), chosen.value.clone())
    };

    let client = reqwest::Client::builder()
        .connect_timeout(FETCH_TIMEOUT)
        .timeout(FETCH_TIMEOUT)
        // An Authorization header must not follow a redirect to somewhere else.
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("agentlauncher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("cannot build HTTP client: {e}"))?;

    // An empty `auth_style` means the row never said, which is now the normal case:
    // the settings form stopped asking. Try the near-universal bearer scheme, and if
    // the provider rejects the credential *shape* (401/403) retry the way Anthropic
    // wants it. One wasted request in the uncommon case, in exchange for a field the
    // user had no way to know the answer to.
    let styles: &[&str] = match auth_style.as_str() {
        "x-api-key" => &["x-api-key"],
        "bearer" => &["bearer"],
        _ => &["bearer", "x-api-key"],
    };
    let mut last = String::new();
    for (i, style) in styles.iter().enumerate() {
        let req = if *style == "x-api-key" {
            // Anthropic's scheme; the version header is required or it 400s.
            client
                .get(url.clone())
                .header("x-api-key", &key)
                .header("anthropic-version", "2023-06-01")
        } else {
            client
                .get(url.clone())
                .header("authorization", format!("Bearer {key}"))
        };
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        if status.is_success() {
            let models = model_names(&read_json(resp).await?);
            if models.is_empty() {
                return Err("响应里没有模型列表".into());
            }
            return Ok(models);
        }
        // The provider's own words help ("invalid api key" vs "no quota"), bounded and
        // rendered as text by the caller. Never echo the key.
        let body = resp.text().await.unwrap_or_default();
        let snippet: String = body.trim().chars().take(200).collect();
        last = format!("{status}: {snippet}");
        let wrong_shape = status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN;
        if !wrong_shape || i + 1 == styles.len() {
            return Err(last);
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_url_respects_an_existing_version_segment() {
        assert_eq!(
            models_url("https://api.openai.com/v1").unwrap().as_str(),
            "https://api.openai.com/v1/models"
        );
        assert_eq!(
            models_url("https://api.openai.com/v1/").unwrap().as_str(),
            "https://api.openai.com/v1/models"
        );
        // No version segment ⇒ the OpenAI-compatible default is appended.
        assert_eq!(
            models_url("https://api.deepseek.com").unwrap().as_str(),
            "https://api.deepseek.com/v1/models"
        );
        // A gateway on its own version scheme is not given a second one.
        assert_eq!(
            models_url("https://gw.example.com/api/v3").unwrap().as_str(),
            "https://gw.example.com/api/v3/models"
        );
    }

    #[test]
    fn a_key_is_never_sent_in_the_clear_off_this_machine() {
        assert!(models_url("http://api.example.com/v1").is_err());
        // Loopback is the exception: a local runtime has no TLS and no network hop.
        assert!(models_url("http://127.0.0.1:1234/v1").is_ok());
        assert!(models_url("http://localhost:11434/v1").is_ok());
        assert!(models_url("").is_err());
    }

    #[test]
    fn both_models_list_shapes_parse_and_junk_does_not() {
        let openai = serde_json::json!({"data": [{"id": "gpt-4o"}, {"id": "o3-mini"}]});
        assert_eq!(model_names(&openai), vec!["gpt-4o", "o3-mini"]);
        let ollama = serde_json::json!({"models": [{"name": "qwen3:8b"}]});
        assert_eq!(model_names(&ollama), vec!["qwen3:8b"]);
        // What a dev server or an error page yields: nothing, which is how the probe
        // rejects a port that is not an LLM.
        assert!(model_names(&serde_json::json!({"error": "nope"})).is_empty());
        assert!(model_names(&serde_json::json!([1, 2, 3])).is_empty());
    }
}
