use comrak::{markdown_to_html, ComrakOptions};
use lru::LruCache;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::cell::Cell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

use crate::theme::{Theme, Themeable};

const CACHE_CAPACITY: usize = 500;
const FAST_PATH_THRESHOLD: usize = 500;

static MARKDOWN_CACHE: Mutex<Option<LruCache<u64, Vec<Line<'static>>>>> =
    Mutex::new(None);

static SYNTAX_SET: std::sync::LazyLock<SyntaxSet> =
    std::sync::LazyLock::new(SyntaxSet::load_defaults_newlines);

static THEME_SET: std::sync::LazyLock<ThemeSet> =
    std::sync::LazyLock::new(ThemeSet::load_defaults);

fn get_cache() -> std::sync::MutexGuard<'static, Option<LruCache<u64, Vec<Line<'static>>>>> {
    MARKDOWN_CACHE.lock().unwrap()
}

fn cache_key(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

fn has_markdown_syntax(content: &str) -> bool {
    let check_len = content.len().min(FAST_PATH_THRESHOLD);
    let prefix = &content[..check_len];

    prefix.contains('#')
        || prefix.contains("**")
        || prefix.contains("__")
        || prefix.contains('*')
        || prefix.contains('_')
        || prefix.contains('`')
        || prefix.contains('[')
        || prefix.contains("![")
        || prefix.contains("---")
        || prefix.contains("***")
        || prefix.contains("```")
        || prefix.contains("> ")
        || prefix.contains("- ")
        || prefix.contains("* ")
        || prefix.contains("+ ")
        || prefix.contains("1. ")
        || prefix.contains("2. ")
        || prefix.contains("3. ")
}

fn parse_markdown_to_spans(content: &str, theme: &Theme) -> Vec<Line<'static>> {
    if content.is_empty() {
        return vec![Line::from(Span::raw(""))];
    }

    if !has_markdown_syntax(content) {
        return vec![Line::from(Span::styled(
            content.to_string(),
            Style::default().fg(theme.colors.text),
        ))];
    }

    let key = cache_key(content);
    {
        let mut cache = get_cache();
        if let Some(ref mut lru) = *cache {
            if let Some(cached) = lru.get(&key) {
                return cached.clone();
            }
        }
    }

    let html = markdown_to_html(content, &default_comrak_options());
    let lines = html_to_spans(&html, theme);

    {
        let mut cache = get_cache();
        if cache.is_none() {
            *cache = Some(LruCache::new(
                std::num::NonZeroUsize::new(CACHE_CAPACITY).unwrap(),
            ));
        }
        if let Some(ref mut lru) = *cache {
            lru.put(key, lines.clone());
        }
    }

    lines
}

fn default_comrak_options() -> ComrakOptions {
    let mut options = ComrakOptions::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.render.hardbreaks = true;
    options.render.unsafe_ = true;
    options
}

fn html_to_spans(html: &str, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut current_spans = Vec::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_content = String::new();
    let mut in_heading = false;
    let mut heading_level = 0;
    let mut in_bold = false;
    let mut in_italic = false;
    let mut in_strikethrough = false;
    let mut in_link = false;
    let mut link_text = String::new();
    let mut text_buffer = String::new();

    let mut chars = html.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            if !text_buffer.is_empty() {
                let style = compute_style(
                    theme,
                    in_code_block,
                    in_heading,
                    heading_level,
                    in_bold,
                    in_italic,
                    in_strikethrough,
                    in_link,
                );
                current_spans.push(Span::styled(text_buffer.clone(), style));
                text_buffer.clear();
            }

            let mut tag = String::new();
            while let Some(&nc) = chars.peek() {
                chars.next();
                if nc == '>' {
                    break;
                }
                tag.push(nc);
            }

            let tag_lower = tag.to_lowercase();

            if tag_lower.starts_with("pre") {
                if tag_lower.starts_with("/pre") {
                    if !code_content.is_empty() {
                        let code_lines = highlight_code(&code_content, &code_lang, theme);
                        lines.extend(code_lines);
                        code_content.clear();
                    }
                    in_code_block = false;
                } else {
                    in_code_block = true;
                }
            } else if tag_lower.starts_with("code") {
                if !in_code_block && tag_lower.starts_with("code") {
                    if let Some(lang) = extract_language(&tag) {
                        code_lang = lang;
                    }
                }
            } else if tag_lower.starts_with("h") && tag_lower.len() >= 2 && tag_lower.chars().nth(1).map_or(false, |c| c.is_ascii_digit()) {
                if tag_lower.starts_with("/h") {
                    if !text_buffer.is_empty() {
                        let style = compute_style(
                            theme, in_code_block, true, heading_level,
                            in_bold, in_italic, in_strikethrough, in_link,
                        );
                        current_spans.push(Span::styled(text_buffer.clone(), style));
                        text_buffer.clear();
                    }
                    lines.push(Line::from(current_spans.clone()));
                    current_spans.clear();
                    in_heading = false;
                } else {
                    if let Some(level) = tag_lower.chars().nth(1).and_then(|c| c.to_digit(10)) {
                        heading_level = level as usize;
                        in_heading = true;
                    }
                }
            } else if tag_lower.starts_with("strong") || tag_lower.starts_with("b") {
                if tag_lower.starts_with('/') {
                    in_bold = false;
                } else {
                    in_bold = true;
                }
            } else if tag_lower.starts_with("em") || tag_lower.starts_with("i") {
                if tag_lower.starts_with('/') {
                    in_italic = false;
                } else {
                    in_italic = true;
                }
            } else if tag_lower.starts_with("del") || tag_lower.starts_with("s") {
                if tag_lower.starts_with('/') {
                    in_strikethrough = false;
                } else {
                    in_strikethrough = true;
                }
            } else if tag_lower.starts_with("a") {
                if tag_lower.starts_with("/a") {
                    in_link = false;
                    if !link_text.is_empty() {
                        let style = Style::default()
                            .fg(theme.colors.suggestion)
                            .add_modifier(Modifier::UNDERLINED);
                        current_spans.push(Span::styled(link_text.clone(), style));
                        link_text.clear();
                    }
                } else {
                    in_link = true;
                }
            } else if tag_lower == "br" {
                if !text_buffer.is_empty() {
                    let style = compute_style(
                        theme, in_code_block, in_heading, heading_level,
                        in_bold, in_italic, in_strikethrough, in_link,
                    );
                    current_spans.push(Span::styled(text_buffer.clone(), style));
                    text_buffer.clear();
                }
                lines.push(Line::from(current_spans.clone()));
                current_spans.clear();
            } else if tag_lower.starts_with("/p") || tag_lower.starts_with("/div") {
                if !text_buffer.is_empty() {
                    let style = compute_style(
                        theme, in_code_block, in_heading, heading_level,
                        in_bold, in_italic, in_strikethrough, in_link,
                    );
                    current_spans.push(Span::styled(text_buffer.clone(), style));
                    text_buffer.clear();
                }
                if !current_spans.is_empty() {
                    lines.push(Line::from(current_spans.clone()));
                    current_spans.clear();
                }
            } else if tag_lower.starts_with("li") {
                if !text_buffer.is_empty() {
                    let style = compute_style(
                        theme, in_code_block, in_heading, heading_level,
                        in_bold, in_italic, in_strikethrough, in_link,
                    );
                    current_spans.push(Span::styled(text_buffer.clone(), style));
                    text_buffer.clear();
                }
                if !current_spans.is_empty() {
                    lines.push(Line::from(current_spans.clone()));
                    current_spans.clear();
                }
            }
        } else {
            if in_code_block {
                code_content.push(c);
            } else if in_link {
                link_text.push(c);
            } else {
                text_buffer.push(c);
            }
        }
    }

    if !text_buffer.is_empty() {
        let style = compute_style(
            theme, in_code_block, in_heading, heading_level,
            in_bold, in_italic, in_strikethrough, in_link,
        );
        current_spans.push(Span::styled(text_buffer, style));
    }

    if !current_spans.is_empty() {
        lines.push(Line::from(current_spans));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            content_fallback(),
            Style::default().fg(theme.colors.text),
        )));
    }

    lines
}

fn content_fallback() -> String {
    String::new()
}

fn extract_language(tag: &str) -> Option<String> {
    if let Some(start) = tag.find("class=\"language-") {
        let rest = &tag[start + "class=\"language-".len()..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].to_string());
        }
    }
    None
}

fn highlight_code(code: &str, lang: &str, theme: &Theme) -> Vec<Line<'static>> {
    let ps = &*SYNTAX_SET;
    let ts = &*THEME_SET;

    let syntax = ps
        .find_syntax_by_name(lang)
        .or_else(|| ps.find_syntax_by_extension(lang))
        .unwrap_or_else(|| ps.find_syntax_plain_text());

    let syntect_theme = if theme.name == crate::theme::ThemeName::Dark {
        &ts.themes["base16-ocean.dark"]
    } else {
        &ts.themes["base16-ocean.light"]
    };

    let mut highlighter = HighlightLines::new(syntax, syntect_theme);
    let regions = highlighter.highlight_line(code, ps).unwrap_or_default();

    let mut lines = Vec::new();
    let mut spans = Vec::new();

    for &(ref style, text) in &regions {
        let color = syntect_color_to_ratatui(style.foreground);
        let span_style = Style::default().fg(color);
        spans.push(Span::styled(text.to_string(), span_style));
    }

    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            code.to_string(),
            Style::default().fg(theme.colors.text),
        )));
    }

    lines
}

fn syntect_color_to_ratatui(color: syntect::highlighting::Color) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

fn compute_style(
    theme: &Theme,
    in_code_block: bool,
    in_heading: bool,
    heading_level: usize,
    in_bold: bool,
    in_italic: bool,
    in_strikethrough: bool,
    in_link: bool,
) -> Style {
    if in_code_block {
        return Style::default().fg(Color::Cyan);
    }

    let mut style = Style::default().fg(theme.colors.text);

    if in_heading {
        match heading_level {
            1 => style = style.fg(theme.colors.suggestion).add_modifier(Modifier::BOLD),
            2 => style = style.fg(theme.colors.text).add_modifier(Modifier::BOLD),
            3 => style = style.fg(theme.colors.text).add_modifier(Modifier::BOLD),
            _ => style = style.add_modifier(Modifier::BOLD),
        }
    }

    if in_bold {
        style = style.add_modifier(Modifier::BOLD);
    }

    if in_italic {
        style = style.add_modifier(Modifier::ITALIC);
    }

    if in_strikethrough {
        style = style.add_modifier(Modifier::CROSSED_OUT);
    }

    if in_link {
        style = style
            .fg(theme.colors.suggestion)
            .add_modifier(Modifier::UNDERLINED);
    }

    style
}

pub struct MarkdownWidget {
    content: String,
}

impl MarkdownWidget {
    pub fn new(content: String) -> Self {
        Self { content }
    }

    pub fn render_lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        parse_markdown_to_spans(&self.content, theme)
    }
}

impl Themeable for MarkdownWidget {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        let lines = self.render_lines(theme);
        let max_lines = area.height as usize;
        let display_lines: Vec<Line> = lines.into_iter().take(max_lines).collect();

        let y_end = (area.y + area.height).min(buf.area.height);
        for (i, line) in display_lines.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.y && y < y_end {
                for (x_offset, span) in line.spans.iter().enumerate() {
                    let x = area.x + x_offset as u16;
                    if x < buf.area.width {
                        let cell = buf.cell_mut((x, y)).unwrap();
                        cell.set_symbol(&span.content);
                        cell.set_style(span.style);
                    }
                }
            }
        }
    }
}

impl Widget for MarkdownWidget {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let theme = Theme::dark();
        self.render_themed(area, buf, &theme);
    }
}

pub struct StreamingMarkdown {
    stable_prefix: String,
    unstable_suffix: String,
    last_rendered_hash: Cell<u64>,
}

impl StreamingMarkdown {
    pub fn new() -> Self {
        Self {
            stable_prefix: String::new(),
            unstable_suffix: String::new(),
            last_rendered_hash: Cell::new(0),
        }
    }

    pub fn push(&mut self, chunk: &str) {
        self.unstable_suffix.push_str(chunk);
    }

    pub fn commit_stable(&mut self) {
        if !self.unstable_suffix.is_empty() {
            self.stable_prefix.push_str(&self.unstable_suffix);
            self.unstable_suffix.clear();
        }
    }

    pub fn full_content(&self) -> String {
        let mut content = self.stable_prefix.clone();
        content.push_str(&self.unstable_suffix);
        content
    }

    pub fn render_lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let content = self.full_content();
        parse_markdown_to_spans(&content, theme)
    }

    pub fn needs_rerender(&self) -> bool {
        let content = self.full_content();
        let current_hash = cache_key(&content);
        let changed = current_hash != self.last_rendered_hash.get();
        self.last_rendered_hash.set(current_hash);
        changed
    }

    pub fn stable_prefix_len(&self) -> usize {
        self.stable_prefix.len()
    }
}

impl Default for StreamingMarkdown {
    fn default() -> Self {
        Self::new()
    }
}
