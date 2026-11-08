//! Optional AI rewrite pass: sends hard files to the Anthropic Messages API or
//! an OpenAI-compatible /chat/completions endpoint and parses the rewritten
//! file out of a fenced code block.

use crate::model::{AiProvider, AiSettings, Answers, Finding, Os};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

pub const DEFAULT_ANTHROPIC_MODEL: &str = "claude-opus-5-5";
const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const MAX_TOKENS: u32 = 16000;

/// Is the AI pass configured and usable?
pub fn available(ai: &AiSettings) -> bool {
    ai.enabled && !ai.api_key.trim().is_empty() && ai.provider != AiProvider::None
}

pub struct AiRequest<'a> {
    pub path: &'a str,
    pub language: &'a str,
    pub content: &'a str,
    pub findings: &'a [Finding],
    pub targets: &'a [Os],
    pub source_os: &'a [Os],
    pub answers: &'a Answers,
}

const SYSTEM: &str = "You are Crossfire, an expert systems programmer who ports software between Windows, macOS, Linux and FreeBSD. \
You rewrite one source file at a time so it builds and behaves correctly on every requested target OS. \
Preserve behavior, public interfaces (function names, signatures, exported symbols) and comments. \
Prefer the portability choices the user selected; keep native code paths behind #if/#ifdef (or the language's equivalent) only when no portable option exists. \
The project ships a helper header crossfire/cf_platform.h with: cf_sleep_ms, cf_sleep_us, cf_ticks_ms, cf_ticks_ns, cf_stricmp, cf_strnicmp, cf_strlcpy, \
cf_user_data_dir, cf_home_dir, cf_temp_dir, cf_executable_path, cf_resource_dir, cf_open_url, cf_message_box, cf_dlopen/cf_dlsym/cf_dlclose, cf_aligned_alloc/cf_aligned_free, \
cf_getch, cf_kbhit, cf_set_thread_name, cf_net_init, cf_closesocket, macros CF_EXPORT, CF_FORCEINLINE, CF_ALIGN(n), CF_OS_WINDOWS/CF_OS_MACOS/CF_OS_LINUX/CF_OS_FREEBSD, \
and C++ wrappers in namespace cf (cf::sleep_ms, cf::ticks_ms, ...). Python projects have crossfire.cf_platform and crossfire.cf_winreg; .NET projects have Crossfire.CfPlatform and Crossfire.CfSettings. \
Return ONLY the complete rewritten file inside a single fenced code block, with no explanation before or after.";

fn answers_summary(a: &Answers) -> String {
    let mut keys: Vec<&String> = a.keys().collect();
    keys.sort();
    keys.iter()
        .filter(|k| !["useAi", "keepOriginals", "ci", "packages", "bundleId"].contains(&k.as_str()))
        .map(|k| format!("- {k}: {}", a[*k]))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn build_prompt(r: &AiRequest) -> String {
    let targets = r
        .targets
        .iter()
        .map(|o| o.label())
        .collect::<Vec<_>>()
        .join(", ");
    let source = r
        .source_os
        .iter()
        .map(|o| o.label())
        .collect::<Vec<_>>()
        .join(", ");
    let mut issues = String::new();
    for f in r.findings.iter().take(80) {
        let sugg = r
            .targets
            .iter()
            .filter_map(|t| f.suggestions.get(t).map(|s| format!("{}: {s}", t.label())))
            .collect::<Vec<_>>()
            .join(" | ");
        issues.push_str(&format!(
            "- line {} [{:?}] {} — {} Suggestions: {}\n",
            f.line, f.severity, f.api, f.message, sugg
        ));
    }
    format!(
        "Port this {lang} file from {source} so it works on: {targets}.\n\nChosen options:\n{answers}\n\nPlatform issues found by static analysis:\n{issues}\nFile `{path}`:\n```{fence}\n{content}\n```\n\nRewrite the whole file. Return ONLY the full file in a single fenced code block.",
        lang = r.language,
        source = if source.is_empty() { "its original OS".to_string() } else { source },
        answers = answers_summary(r.answers),
        path = r.path,
        fence = fence_lang(r.language),
        content = r.content,
    )
}

fn fence_lang(lang: &str) -> &'static str {
    match lang {
        "C" => "c",
        "C++" | "C/C++ Header" => "cpp",
        "Objective-C" => "objc",
        "Objective-C++" => "objcpp",
        "C#" => "csharp",
        "Python" => "python",
        "Rust" => "rust",
        "Go" => "go",
        "Swift" => "swift",
        "JavaScript" => "js",
        "TypeScript" => "ts",
        _ => "",
    }
}

/// Extract the first fenced code block (```lang ... ```) from a reply.
pub fn extract_code_block(reply: &str) -> Option<String> {
    let start = reply.find("```")?;
    let after = &reply[start + 3..];
    let nl = after.find('\n')?;
    let body = &after[nl + 1..];
    // Closing fence must start a line.
    let mut end = None;
    let mut offset = 0;
    for line in body.split_inclusive('\n') {
        if line.trim_start().starts_with("```") {
            end = Some(offset);
            break;
        }
        offset += line.len();
    }
    let code = &body[..end.unwrap_or(body.len())];
    let code = code.strip_suffix('\n').unwrap_or(code);
    if code.trim().is_empty() {
        None
    } else {
        Some(format!("{code}\n"))
    }
}

/// Call the configured model and return the rewritten file content.
pub fn rewrite(ai: &AiSettings, req: &AiRequest) -> Result<String> {
    let prompt = build_prompt(req);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .context("creating HTTP client")?;
    let reply = match ai.provider {
        AiProvider::Anthropic => {
            let model = if ai.model.trim().is_empty() {
                DEFAULT_ANTHROPIC_MODEL
            } else {
                ai.model.trim()
            };
            let url = ai
                .base_url
                .as_deref()
                .filter(|u| !u.trim().is_empty())
                .map(|u| {
                    format!(
                        "{}/v1/messages",
                        u.trim_end_matches('/').trim_end_matches("/v1")
                    )
                })
                .unwrap_or_else(|| ANTHROPIC_URL.to_string());
            let body = json!({
                "model": model,
                "max_tokens": MAX_TOKENS,
                "system": SYSTEM,
                "messages": [{"role": "user", "content": prompt}],
            });
            let resp = client
                .post(url)
                .header("x-api-key", ai.api_key.trim())
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .context("calling Anthropic API")?;
            let status = resp.status();
            let v: Value = resp.json().context("decoding Anthropic response")?;
            if !status.is_success() {
                bail!(
                    "Anthropic API {status}: {}",
                    v.pointer("/error/message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown error")
                );
            }
            match v.get("stop_reason").and_then(|s| s.as_str()) {
                Some("max_tokens") => {
                    bail!("response truncated at max_tokens (file too large for one pass)")
                }
                Some("refusal") => bail!(
                    "model declined the request ({})",
                    v.pointer("/stop_details/category")
                        .and_then(|c| c.as_str())
                        .unwrap_or("no category")
                ),
                _ => {}
            }
            // Only `text` blocks carry the answer; thinking blocks are skipped.
            v.get("content")
                .and_then(|c| c.as_array())
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                        .collect::<Vec<_>>()
                        .join("")
                })
                .ok_or_else(|| anyhow!("no text content in response"))?
        }
        AiProvider::OpenaiCompatible => {
            let base = ai
                .base_url
                .as_deref()
                .filter(|u| !u.trim().is_empty())
                .unwrap_or("https://api.openai.com/v1");
            if ai.model.trim().is_empty() {
                bail!("no model configured for the OpenAI-compatible provider");
            }
            let url = format!("{}/chat/completions", base.trim_end_matches('/'));
            let body = json!({
                "model": ai.model.trim(),
                "max_tokens": MAX_TOKENS,
                "messages": [
                    {"role": "system", "content": SYSTEM},
                    {"role": "user", "content": prompt}
                ],
            });
            let resp = client
                .post(url)
                .bearer_auth(ai.api_key.trim())
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .context("calling chat completions endpoint")?;
            let status = resp.status();
            let v: Value = resp.json().context("decoding chat completions response")?;
            if !status.is_success() {
                bail!(
                    "endpoint returned {status}: {}",
                    v.pointer("/error/message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown error")
                );
            }
            if v.pointer("/choices/0/finish_reason")
                .and_then(|s| s.as_str())
                == Some("length")
            {
                bail!("response truncated (finish_reason=length)");
            }
            v.pointer("/choices/0/message/content")
                .and_then(|c| c.as_str())
                .map(str::to_string)
                .ok_or_else(|| anyhow!("no message content in response"))?
        }
        AiProvider::None => bail!("AI provider is 'none'"),
    };
    extract_code_block(&reply)
        .ok_or_else(|| anyhow!("model reply did not contain a fenced code block"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_fenced_block() {
        let r = "Here you go:\n```cpp\nint main() {\n  return 0;\n}\n```\nDone.";
        assert_eq!(
            extract_code_block(r).unwrap(),
            "int main() {\n  return 0;\n}\n"
        );
        assert!(extract_code_block("no code").is_none());
        // Unterminated fence still yields the body.
        assert_eq!(extract_code_block("```\nx\n").unwrap(), "x\n");
    }

    #[test]
    fn availability_requires_key() {
        let mut ai = AiSettings {
            enabled: true,
            provider: AiProvider::Anthropic,
            ..Default::default()
        };
        assert!(!available(&ai));
        ai.api_key = "k".into();
        assert!(available(&ai));
    }
}
