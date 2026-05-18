use std::sync::Arc;

use async_trait::async_trait;
use cc_core::messages::ContentBlockParam;
use cc_core::permissions::PermissionResult;
use cc_core::tools::{
    InterruptBehavior, SearchOrReadInfo, Tool, ToolProgress, ToolPromptOptions, ToolResult,
    ToolUseContext,
};
use cc_core::types::ValidationResult;
use html2text::from_read;
use lru::LruCache;
use tokio::sync::Mutex;
use url::Url;

use crate::utils::check_read_permission;

/// Max URL length (2000 chars).
const MAX_URL_LENGTH: usize = 2000;

/// Max HTTP content length (10MB).
const MAX_HTTP_CONTENT_LENGTH: usize = 10 * 1024 * 1024;

/// Fetch timeout (60 seconds).
const FETCH_TIMEOUT_MS: u64 = 60_000;

/// Max redirects to follow.
const MAX_REDIRECTS: usize = 10;

/// Max markdown content length before truncation.
const MAX_MARKDOWN_LENGTH: usize = 100_000;

/// Cache TTL in milliseconds (15 minutes).
const CACHE_TTL_MS: u64 = 15 * 60 * 1000;

/// Cached web fetch entry.
#[derive(Clone)]
struct CacheEntry {
    content: String,
    bytes: usize,
    code: u16,
    code_text: String,
    content_type: String,
    fetched_at: std::time::Instant,
}

/// Preapproved hosts that bypass LLM summarization.
fn is_preapproved_host(hostname: &str, pathname: &str) -> bool {
    const HOSTNAMES: &[&str] = &[
        "platform.claude.com",
        "code.claude.com",
        "modelcontextprotocol.io",
        "agentskills.io",
        "docs.python.org",
        "en.cppreference.com",
        "docs.oracle.com",
        "learn.microsoft.com",
        "developer.mozilla.org",
        "go.dev",
        "pkg.go.dev",
        "www.php.net",
        "docs.swift.org",
        "kotlinlang.org",
        "ruby-doc.org",
        "doc.rust-lang.org",
        "www.typescriptlang.org",
        "react.dev",
        "angular.io",
        "vuejs.org",
        "nextjs.org",
        "expressjs.com",
        "nodejs.org",
        "bun.sh",
        "jquery.com",
        "getbootstrap.com",
        "tailwindcss.com",
        "d3js.org",
        "threejs.org",
        "redux.js.org",
        "webpack.js.org",
        "jestjs.io",
        "reactrouter.com",
        "docs.djangoproject.com",
        "flask.palletsprojects.com",
        "fastapi.tiangolo.com",
        "pandas.pydata.org",
        "numpy.org",
        "www.tensorflow.org",
        "pytorch.org",
        "scikit-learn.org",
        "matplotlib.org",
        "requests.readthedocs.io",
        "jupyter.org",
        "laravel.com",
        "symfony.com",
        "wordpress.org",
        "docs.spring.io",
        "hibernate.org",
        "tomcat.apache.org",
        "gradle.org",
        "maven.apache.org",
        "asp.net",
        "dotnet.microsoft.com",
        "nuget.org",
        "blazor.net",
        "reactnative.dev",
        "docs.flutter.dev",
        "developer.apple.com",
        "developer.android.com",
        "keras.io",
        "spark.apache.org",
        "huggingface.co",
        "www.kaggle.com",
        "www.mongodb.com",
        "redis.io",
        "www.postgresql.org",
        "dev.mysql.com",
        "www.sqlite.org",
        "graphql.org",
        "prisma.io",
        "docs.aws.amazon.com",
        "cloud.google.com",
        "kubernetes.io",
        "www.docker.com",
        "www.terraform.io",
        "www.ansible.com",
        "docs.netlify.com",
        "devcenter.heroku.com",
        "cypress.io",
        "selenium.dev",
        "docs.unity.com",
        "docs.unrealengine.com",
        "git-scm.com",
        "nginx.org",
        "httpd.apache.org",
    ];

    const PATH_SCOPED: &[(&str, &str)] = &[
        ("github.com", "/anthropics"),
        ("vercel.com", "/docs"),
    ];

    if HOSTNAMES.contains(&hostname) {
        return true;
    }

    for (host, prefix) in PATH_SCOPED {
        if hostname == *host && (pathname == *prefix || pathname.starts_with(&format!("{prefix}/"))) {
            return true;
        }
    }

    false
}

/// Check if a redirect is safe to follow.
fn is_permitted_redirect(original_url: &Url, redirect_url: &Url) -> bool {
    if redirect_url.scheme() != original_url.scheme() {
        return false;
    }
    if redirect_url.port() != original_url.port() {
        return false;
    }
    if !redirect_url.username().is_empty() || redirect_url.password().is_some() {
        return false;
    }

    let orig_host = original_url.host_str().unwrap_or("");
    let redir_host = redirect_url.host_str().unwrap_or("");
    let strip_www = |h: &str| -> String {
        h.strip_prefix("www.").unwrap_or(h).to_string()
    };
    strip_www(orig_host) == strip_www(redir_host)
}

/// Result of fetching content.
enum FetchedContent {
    Content {
        content: String,
        bytes: usize,
        code: u16,
        code_text: String,
        content_type: String,
    },
    Redirect {
        original_url: String,
        redirect_url: String,
        status_code: u16,
    },
}

/// WebFetch tool - fetches content from a URL and processes it.
#[derive(Debug)]
pub struct WebFetchTool {
    cache: Arc<Mutex<LruCache<String, CacheEntry>>>,
    http_client: reqwest::Client,
}

impl WebFetchTool {
    pub fn new() -> Arc<dyn Tool> {
        Arc::new(Self {
            cache: Arc::new(Mutex::new(LruCache::new(
                std::num::NonZeroUsize::new(1024).unwrap(),
            ))),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_millis(FETCH_TIMEOUT_MS))
                .build()
                .expect("Failed to build HTTP client"),
        })
    }

    /// Validate a URL.
    fn validate_url(url_str: &str) -> Result<Url, anyhow::Error> {
        if url_str.len() > MAX_URL_LENGTH {
            return Err(anyhow::anyhow!("URL too long (max {MAX_URL_LENGTH} chars)"));
        }

        let url = Url::parse(url_str).map_err(|e| anyhow::anyhow!("Invalid URL: {e}"))?;

        if !url.username().is_empty() || url.password().is_some() {
            return Err(anyhow::anyhow!("URLs with username/password are not allowed"));
        }

        let hostname = url.host_str().unwrap_or("");
        let parts: Vec<&str> = hostname.split('.').collect();
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Hostname must have at least 2 parts"));
        }

        Ok(url)
    }

    /// Follow redirects with permission checking.
    async fn fetch_with_redirects(
        &self,
        url: &Url,
        depth: usize,
    ) -> Result<FetchedContent, anyhow::Error> {
        if depth > MAX_REDIRECTS {
            return Err(anyhow::anyhow!("Too many redirects (exceeded {MAX_REDIRECTS})"));
        }

        let fetch_url = if url.scheme() == "http" {
            let mut upgraded = url.clone();
            upgraded.set_scheme("https").unwrap();
            upgraded
        } else {
            url.clone()
        };

        let response = self
            .http_client
            .get(fetch_url.clone())
            .header("Accept", "text/markdown, text/html, */*")
            .header("User-Agent", "Claude-Code-Rust/0.1.0")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch URL: {e}"))?;

        // Check for redirect
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get("location")
                .ok_or_else(|| anyhow::anyhow!("Redirect missing Location header"))?;

            let redirect_url = Url::options()
                .base_url(Some(&fetch_url))
                .parse(location.to_str().unwrap_or(""))
                .map_err(|e| anyhow::anyhow!("Invalid redirect URL: {e}"))?;

            if is_permitted_redirect(&fetch_url, &redirect_url) {
                // Box::pin for recursive async call
                return Box::pin(self.fetch_with_redirects(&redirect_url, depth + 1)).await;
            } else {
                return Ok(FetchedContent::Redirect {
                    original_url: fetch_url.to_string(),
                    redirect_url: redirect_url.to_string(),
                    status_code: response.status().as_u16(),
                });
            }
        }

        let status = response.status();
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let bytes = response
            .content_length()
            .unwrap_or(0)
            .min(MAX_HTTP_CONTENT_LENGTH as u64) as usize;

        let body = response
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read response body: {e}"))?;

        let body_str = String::from_utf8_lossy(&body).to_string();

        Ok(FetchedContent::Content {
            content: body_str,
            bytes,
            code: status.as_u16(),
            code_text: status.canonical_reason().unwrap_or("").to_string(),
            content_type,
        })
    }

    /// Convert HTML to markdown.
    fn html_to_markdown(html: &str) -> String {
        let bytes = html.as_bytes();
        let max_width = 120;
        from_read(bytes, max_width).unwrap_or_else(|_| html.to_string())
    }

    /// Check cache for URL.
    async fn get_cached(&self, url: &str) -> Option<CacheEntry> {
        let mut cache = self.cache.lock().await;
        if let Some(entry) = cache.get(url) {
            if (entry.fetched_at.elapsed().as_millis() as u64) < CACHE_TTL_MS {
                return Some(entry.clone());
            }
            cache.pop(url);
        }
        None
    }

    /// Store in cache.
    async fn set_cached(&self, url: &str, entry: CacheEntry) {
        let mut cache = self.cache.lock().await;
        cache.put(url.to_string(), entry);
    }

    /// Process fetched content.
    async fn process_content(
        &self,
        markdown: &str,
        prompt: &str,
        url: &str,
        content_type: &str,
    ) -> String {
        let is_preapproved = if let Ok(parsed) = Url::parse(url) {
            is_preapproved_host(parsed.host_str().unwrap_or(""), parsed.path())
        } else {
            false
        };

        if is_preapproved
            && content_type.contains("text/markdown")
            && markdown.len() < MAX_MARKDOWN_LENGTH
        {
            return markdown.to_string();
        }

        let truncated = if markdown.len() > MAX_MARKDOWN_LENGTH {
            format!(
                "{}\n\n[Content truncated due to length...]",
                &markdown[..MAX_MARKDOWN_LENGTH]
            )
        } else {
            markdown.to_string()
        };

        let guidelines = if is_preapproved {
            "Provide a concise response based on the content above."
        } else {
            "Provide a concise response based only on the content above. Enforce a strict 125-character maximum for quotes."
        };

        let _model_prompt = format!(
            "Web page content:\n---\n{truncated}\n---\n\n{prompt}\n\n{guidelines}"
        );

        // TODO: Integrate with ApiService for secondary model call
        format!(
            "[Content fetched from {url}]\n\n\
             [Note: In production, this content would be summarized via a secondary model call]\n\n\
             Prompt: {prompt}\n\n\
             Content (first {} chars):\n{truncated}",
            MAX_MARKDOWN_LENGTH.min(truncated.len())
        )
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    fn search_hint(&self) -> Option<&str> {
        Some("fetch and extract content from a URL")
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Arc<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> anyhow::Result<ToolResult<serde_json::Value>> {
        let url_str = input["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("url is required"))?;
        let prompt = input["prompt"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("prompt is required"))?;

        let start = std::time::Instant::now();

        let url = Self::validate_url(url_str)?;

        if let Some(entry) = self.get_cached(url_str).await {
            let result = self.process_content(&entry.content, prompt, url_str, &entry.content_type).await;
            let duration_ms = start.elapsed().as_millis() as u64;

            let output = serde_json::json!({
                "bytes": entry.bytes,
                "code": entry.code,
                "codeText": entry.code_text,
                "result": result,
                "durationMs": duration_ms,
                "url": url_str,
            });

            return Ok(ToolResult {
                data: output,
                new_messages: None,
                mcp_meta: None,
            });
        }

        let fetched = self.fetch_with_redirects(&url, 0).await?;

        match fetched {
            FetchedContent::Redirect {
                original_url,
                redirect_url,
                status_code,
            } => {
                let status_text = match status_code {
                    301 => "Moved Permanently",
                    308 => "Permanent Redirect",
                    307 => "Temporary Redirect",
                    _ => "Found",
                };

                let message = format!(
                    "REDIRECT DETECTED: The URL redirects to a different host.\n\n\
                     Original URL: {original_url}\n\
                     Redirect URL: {redirect_url}\n\
                     Status: {status_code} {status_text}\n\n\
                     To complete your request, I need to fetch content from the redirected URL. \
                     Please use WebFetch again with these parameters:\n\
                     - url: \"{redirect_url}\"\n\
                     - prompt: \"{prompt}\""
                );

                let output = serde_json::json!({
                    "bytes": message.len(),
                    "code": status_code,
                    "codeText": status_text,
                    "result": message,
                    "durationMs": start.elapsed().as_millis() as u64,
                    "url": url_str,
                });

                Ok(ToolResult {
                    data: output,
                    new_messages: None,
                    mcp_meta: None,
                })
            }
            FetchedContent::Content {
                content,
                bytes,
                code,
                code_text,
                content_type,
            } => {
                let markdown = if content_type.contains("text/html") {
                    Self::html_to_markdown(&content)
                } else {
                    content
                };

                let cache_entry = CacheEntry {
                    content: markdown.clone(),
                    bytes,
                    code,
                    code_text: code_text.clone(),
                    content_type: content_type.clone(),
                    fetched_at: std::time::Instant::now(),
                };
                self.set_cached(url_str, cache_entry).await;

                let result = self.process_content(&markdown, prompt, url_str, &content_type).await;

                let output = serde_json::json!({
                    "bytes": bytes,
                    "code": code,
                    "codeText": code_text,
                    "result": result,
                    "durationMs": start.elapsed().as_millis() as u64,
                    "url": url_str,
                });

                Ok(ToolResult {
                    data: output,
                    new_messages: None,
                    mcp_meta: None,
                })
            }
        }
    }

    async fn description(
        &self,
        input: serde_json::Value,
        _options: &cc_core::tools::DescriptionOptions,
    ) -> anyhow::Result<String> {
        let url = input["url"].as_str().unwrap_or("this URL");
        if let Ok(parsed) = Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                return Ok(format!("Claude wants to fetch content from {host}"));
            }
        }
        Ok("Claude wants to fetch content from this URL".to_string())
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch content from"
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt to run on the fetched content"
                }
            },
            "required": ["url", "prompt"]
        })
    }

    fn is_concurrency_safe(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn is_read_only(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn is_search_or_read_command(&self, _input: &serde_json::Value) -> SearchOrReadInfo {
        SearchOrReadInfo {
            is_search: false,
            is_read: true,
            is_list: false,
        }
    }

    fn is_open_world(&self, _input: &serde_json::Value) -> bool {
        true
    }

    fn should_defer(&self) -> bool {
        true
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> anyhow::Result<ValidationResult> {
        let url = input["url"].as_str();
        match url {
            None => Ok(ValidationResult::Invalid {
                message: "url is required".to_string(),
                error_code: 1,
            }),
            Some(u) => match Self::validate_url(u) {
                Ok(_) => Ok(ValidationResult::Valid),
                Err(e) => Ok(ValidationResult::Invalid {
                    message: format!("Error: Invalid URL \"{u}\". {e}"),
                    error_code: 1,
                }),
            },
        }
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        context: &ToolUseContext,
    ) -> anyhow::Result<PermissionResult> {
        if let Some(url_str) = input["url"].as_str() {
            if let Ok(url) = Url::parse(url_str) {
                let hostname = url.host_str().unwrap_or("");
                let pathname = url.path();
                if is_preapproved_host(hostname, pathname) {
                    return Ok(PermissionResult::Allow {
                        updated_input: Some(input.clone()),
                        user_modified: None,
                        decision_reason: None,
                        tool_use_id: None,
                        accept_feedback: None,
                        content_blocks: None,
                    });
                }
            }
        }

        check_read_permission(input, context, "web_fetch")
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    async fn prompt(&self, _options: &ToolPromptOptions) -> anyhow::Result<String> {
        Ok(
            "IMPORTANT: WebFetch WILL FAIL for authenticated or private URLs. Before using this tool, \
             check if the URL points to an authenticated service (e.g. Google Docs, Confluence, Jira, GitHub). \
             If so, look for a specialized MCP tool that provides authenticated access.\n\n\
             - Fetches content from a specified URL and processes it using an AI model\n\
             - Takes a URL and a prompt as input\n\
             - Fetches the URL content, converts HTML to markdown\n\
             - Processes the content with the prompt using a small, fast model\n\
             - Returns the model's response about the content\n\
             - Use this tool when you need to retrieve and analyze web content\n\n\
             Usage notes:\n\
             - The URL must be a fully-formed valid URL\n\
             - HTTP URLs will be automatically upgraded to HTTPS\n\
             - The prompt should describe what information you want to extract from the page\n\
             - This tool is read-only and does not modify any files\n\
             - Results may be summarized if the content is very large\n\
             - Includes a self-cleaning 15-minute cache for faster responses\n\
             - When a URL redirects to a different host, the tool will inform you and provide the redirect URL\n\
             - For GitHub URLs, prefer using the gh CLI via Bash instead (e.g. gh pr view, gh issue view, gh api)."
                .to_string(),
        )
    }

    fn user_facing_name(&self, _input: Option<&serde_json::Value>) -> String {
        "Fetch".to_string()
    }

    fn get_activity_description(&self, input: Option<&serde_json::Value>) -> Option<String> {
        if let Some(input) = input {
            if let Some(url) = input["url"].as_str() {
                if let Ok(parsed) = Url::parse(url) {
                    if let Some(host) = parsed.host_str() {
                        return Some(format!("Fetching {host}"));
                    }
                }
            }
        }
        Some("Fetching web page".to_string())
    }

    fn get_tool_use_summary(&self, input: Option<&serde_json::Value>) -> Option<String> {
        if let Some(input) = input {
            if let Some(url) = input["url"].as_str() {
                if let Ok(parsed) = Url::parse(url) {
                    return Some(parsed.host_str().unwrap_or(url).to_string());
                }
            }
        }
        None
    }

    fn map_tool_result_to_block(
        &self,
        content: serde_json::Value,
        tool_use_id: &str,
    ) -> ContentBlockParam {
        let result = content["result"]
            .as_str()
            .unwrap_or("(no result)")
            .to_string();
        ContentBlockParam::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: vec![cc_core::messages::ToolResultContent::Text { text: result }],
            is_error: None,
        }
    }
}
