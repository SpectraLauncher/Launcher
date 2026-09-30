use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use std::path::{Path, PathBuf};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::{paths, AppState};

const MAX_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Toml,
    Json,
    Json5,
    Jsonc,
    Properties,
    Cfg,
    Ini,
    Options,
    Snbt,
    Yaml,
    Hocon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Section,
    Bool,
    Int,
    Float,
    String,
    List,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub path: Vec<String>,
    pub kind: Kind,
    pub value: Json,
    pub comment: Option<String>,
    pub default: Option<Json>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub options: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
    pub rel: String,
    pub path: String,
    pub format: Format,
    pub size: u64,
}

#[derive(Debug, Serialize)]
pub struct ConfigDoc {
    pub format: Format,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Change {
    pub path: Vec<String>,
    pub value: Json,
}

pub fn format_of(rel: &str) -> Option<Format> {
    if rel == "options.txt" {
        return Some(Format::Options);
    }
    let name = rel.rsplit('/').next()?.to_ascii_lowercase();
    Some(match name.rsplit_once('.')?.1 {
        "toml" => Format::Toml,
        "json" => Format::Json,
        "json5" => Format::Json5,
        "jsonc" => Format::Jsonc,
        "properties" => Format::Properties,
        "cfg" => Format::Cfg,
        "ini" => Format::Ini,
        "snbt" => Format::Snbt,
        "yml" | "yaml" => Format::Yaml,
        "conf" => Format::Hocon,
        _ => return None,
    })
}

pub fn allowed_rel(rel: &str) -> bool {
    let safe = !rel.is_empty()
        && !rel.starts_with('/')
        && !rel.contains('\\')
        && !rel.contains(':')
        && rel.split('/').all(|part| !part.is_empty() && part != "." && part != "..");
    let place = rel == "options.txt"
        || rel.starts_with("config/")
        || rel.starts_with("defaultconfigs/")
        || (rel.starts_with("saves/") && rel.split('/').nth(2) == Some("serverconfig"));
    safe && place && !rel.contains("/ftbquests/quests/") && format_of(rel).is_some()
}

fn walk(game: &Path, dir: &Path, depth: u32, out: &mut Vec<ConfigFile>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name().to_ascii_lowercase());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            if depth > 0 {
                walk(game, &path, depth - 1, out);
            }
            continue;
        }
        let Ok(rel) = path.strip_prefix(game) else { continue };
        let rel = rel.to_string_lossy().replace('\\', "/");
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if size > MAX_BYTES || !allowed_rel(&rel) {
            continue;
        }
        if let Some(format) = format_of(&rel) {
            out.push(ConfigFile { path: path.to_string_lossy().into_owned(), rel, format, size });
        }
    }
}

pub fn list(game: &Path) -> Vec<ConfigFile> {
    let mut out = Vec::new();
    let options = game.join("options.txt");
    if let Some(meta) = std::fs::metadata(&options).ok().filter(|m| m.is_file()) {
        out.push(ConfigFile {
            rel: "options.txt".into(),
            path: options.to_string_lossy().into_owned(),
            format: Format::Options,
            size: meta.len(),
        });
    }
    walk(game, &game.join("config"), 4, &mut out);
    walk(game, &game.join("defaultconfigs"), 3, &mut out);
    if let Ok(worlds) = std::fs::read_dir(game.join("saves")) {
        for world in worlds.flatten() {
            walk(game, &world.path().join("serverconfig"), 2, &mut out);
        }
    }
    out
}

struct Hints {
    comment: Option<String>,
    default: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    options: Vec<String>,
}

fn number(raw: &str) -> Option<f64> {
    raw.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

fn range(raw: &str, hints: &mut Hints) {
    let raw = raw.trim().trim_start_matches('[').trim_end_matches(']').trim();
    if let Some((a, b)) = raw.split_once('~') {
        hints.min = number(a);
        hints.max = number(b);
    } else if let Some(rest) = raw.strip_prefix('>') {
        hints.min = number(rest.trim_start_matches('='));
    } else if let Some(rest) = raw.strip_prefix('<') {
        hints.max = number(rest.trim_start_matches('='));
    }
}

fn hint(segment: &str, hints: &mut Hints) -> bool {
    let lower = segment.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("default:") {
        hints.default = Some(segment[segment.len() - rest.len()..].trim().to_string());
    } else if lower.starts_with("range:") {
        range(&segment["range:".len()..], hints);
    } else if lower.starts_with("allowed values:") {
        hints.options = segment["allowed values:".len()..]
            .split(',')
            .map(|o| o.trim().to_string())
            .filter(|o| !o.is_empty())
            .collect();
    } else {
        return false;
    }
    true
}

fn hints(lines: &[String]) -> Hints {
    let mut out = Hints { comment: None, default: None, min: None, max: None, options: Vec::new() };
    let mut text = Vec::new();
    for line in lines {
        let mut line = line.trim().to_string();
        if let (Some(open), true) = (line.rfind('['), line.ends_with(']')) {
            let inner = line[open + 1..line.len() - 1].to_string();
            if inner.to_ascii_lowercase().contains("default:") {
                for part in inner.split(',') {
                    hint(part.trim(), &mut out);
                }
                line = line[..open].trim_end().to_string();
            }
        }
        let segments: Vec<&str> = line.split("; ").map(str::trim).collect();
        let mut rest = Vec::new();
        for segment in segments {
            if !hint(segment, &mut out) {
                rest.push(segment);
            }
        }
        let rest = rest.join("; ");
        if !rest.is_empty() {
            text.push(rest);
        }
    }
    let joined = text.join("\n").trim().to_string();
    out.comment = (!joined.is_empty()).then_some(joined);
    out
}

fn typed(kind: Kind, raw: &str) -> Option<Json> {
    let raw = raw.trim();
    match kind {
        Kind::Bool => raw.parse::<bool>().ok().map(Json::from),
        Kind::Int => raw.parse::<i64>().ok().map(Json::from),
        Kind::Float => number(raw).and_then(serde_json::Number::from_f64).map(Json::Number),
        Kind::String => Some(Json::from(raw.trim_matches('"'))),
        _ => None,
    }
}

fn entry(path: Vec<String>, kind: Kind, value: Json, comment: &[String]) -> Entry {
    let hints = hints(comment);
    Entry {
        path,
        kind,
        value,
        default: hints.default.as_deref().and_then(|d| typed(kind, d)),
        comment: hints.comment,
        min: hints.min,
        max: hints.max,
        options: if kind == Kind::String { hints.options } else { Vec::new() },
    }
}

fn comment_lines(raw: &str, markers: &[char]) -> Vec<String> {
    let mut out = Vec::new();
    for line in raw.trim_end_matches([' ', '\t']).lines() {
        let t = line.trim();
        if t.is_empty() {
            out.clear();
            continue;
        }
        if let Some(first) = t.chars().next().filter(|c| markers.contains(c)) {
            let rest = t[first.len_utf8()..].trim_start_matches(first);
            out.push(rest.strip_prefix(' ').unwrap_or(rest).trim_end().to_string());
        }
    }
    out
}

fn raw_text<'a>(src: &'a str, raw: Option<&toml_edit::RawString>) -> &'a str {
    raw.and_then(|r| r.span()).and_then(|span| src.get(span)).unwrap_or("")
}

fn toml_scalar(v: &toml_edit::Value) -> Option<(Kind, Json)> {
    match v {
        toml_edit::Value::Boolean(b) => Some((Kind::Bool, json!(*b.value()))),
        toml_edit::Value::Integer(i) => Some((Kind::Int, json!(*i.value()))),
        toml_edit::Value::Float(f) => serde_json::Number::from_f64(*f.value()).map(|n| (Kind::Float, Json::Number(n))),
        toml_edit::Value::String(s) => Some((Kind::String, json!(s.value()))),
        _ => None,
    }
}

fn toml_value(v: &toml_edit::Value) -> (Kind, Json) {
    if let Some(scalar) = toml_scalar(v) {
        return scalar;
    }
    if let toml_edit::Value::Array(items) = v {
        let values: Option<Vec<Json>> = items.iter().map(|i| toml_scalar(i).map(|(_, j)| j)).collect();
        if let Some(values) = values {
            return (Kind::List, Json::Array(values));
        }
    }
    (Kind::Other, Json::Null)
}

fn walk_toml(src: &str, table: &toml_edit::Table, path: &[String], out: &mut Vec<Node>) {
    for (key, item) in table.iter() {
        let mut at = path.to_vec();
        at.push(key.to_string());
        let decor = table.key(key).map(|k| k.leaf_decor());
        match item {
            toml_edit::Item::Value(v) => {
                let (kind, value) = toml_value(v);
                let Some(span) = v.span() else { continue };
                let comment = comment_lines(raw_text(src, decor.and_then(|d| d.prefix())), &['#']);
                out.push(Node::new(at, kind, value, (span.start, span.end), comment));
            }
            toml_edit::Item::Table(t) => {
                let comment = comment_lines(raw_text(src, t.decor().prefix()), &['#']);
                out.push(Node::new(at.clone(), Kind::Section, Json::Null, (0, 0), comment));
                walk_toml(src, t, &at, out);
            }
            _ => {}
        }
    }
}

fn parse_toml(src: &str) -> AppResult<Vec<Node>> {
    let doc = toml_edit::Document::parse(src).map_err(|e| AppError::invalid(format!("the file is not valid TOML: {e}")))?;
    let table = doc.as_item().as_table().ok_or_else(|| AppError::invalid("the file is not valid TOML"))?;
    let mut out = Vec::new();
    walk_toml(src, table, &[], &mut out);
    Ok(out)
}

#[derive(Debug, Clone)]
struct Node {
    path: Vec<String>,
    kind: Kind,
    value: Json,
    span: (usize, usize),
    comment: Vec<String>,
    quoted: bool,
    affix: String,
}

impl Node {
    fn new(path: Vec<String>, kind: Kind, value: Json, span: (usize, usize), comment: Vec<String>) -> Self {
        Node { path, kind, value, span, comment, quoted: false, affix: String::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dialect {
    Json,
    Snbt,
    Hocon,
}

struct JsonParser<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
    nodes: Vec<Node>,
    comments: Vec<String>,
    dialect: Dialect,
}

impl<'a> JsonParser<'a> {
    fn fail<T>(&self, what: &str) -> AppResult<T> {
        let line = self.src[..self.i.min(self.src.len())].matches('\n').count() + 1;
        let name = match self.dialect {
            Dialect::Json => "JSON",
            Dialect::Snbt => "SNBT",
            Dialect::Hocon => "HOCON",
        };
        Err(AppError::invalid(format!("the file is not valid {name} ({what} on line {line})")))
    }

    fn hash_comments(&self) -> bool {
        self.dialect != Dialect::Json
    }

    fn line_end(&self) -> usize {
        self.src[self.i..].find('\n').map_or(self.s.len(), |n| self.i + n)
    }

    fn skip(&mut self) {
        loop {
            let mut newlines = 0;
            while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
                newlines += usize::from(self.s[self.i] == b'\n');
                self.i += 1;
            }
            if newlines > 1 {
                self.comments.clear();
            }
            if self.s[self.i..].starts_with(b"//") {
                let end = self.line_end();
                self.comments.push(self.src[self.i + 2..end].trim().to_string());
                self.i = end;
            } else if self.hash_comments() && self.s.get(self.i) == Some(&b'#') {
                let end = self.line_end();
                self.comments.extend(comment_lines(&self.src[self.i..end], &['#']));
                self.i = end;
            } else if self.s[self.i..].starts_with(b"/*") {
                let end = self.src[self.i + 2..].find("*/").map_or(self.s.len(), |n| self.i + 2 + n);
                for line in self.src[self.i + 2..end].lines() {
                    let line = line.trim().trim_start_matches('*').trim();
                    if !line.is_empty() {
                        self.comments.push(line.to_string());
                    }
                }
                self.i = (end + 2).min(self.s.len());
            } else {
                break;
            }
        }
    }

    fn string(&mut self) -> AppResult<String> {
        let quote = self.s[self.i];
        self.i += 1;
        let mut out = String::new();
        let mut chars = self.src[self.i..].char_indices();
        while let Some((at, c)) = chars.next() {
            if c as u32 == quote as u32 {
                self.i += at + 1;
                return Ok(out);
            }
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next().map(|(_, c)| c) {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('b') => out.push('\u{8}'),
                Some('f') => out.push('\u{c}'),
                Some('u') => {
                    let hex: String = (0..4).filter_map(|_| chars.next().map(|(_, c)| c)).collect();
                    out.push(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32).unwrap_or('\u{fffd}'));
                }
                Some('\n') => {}
                Some(other) => out.push(other),
                None => break,
            }
        }
        self.fail("an unfinished string")
    }

    fn word(&mut self) -> &'a str {
        let start = self.i;
        while self.i < self.s.len() && (self.s[self.i].is_ascii_alphanumeric() || b"_$+-.".contains(&self.s[self.i])) {
            self.i += 1;
        }
        &self.src[start..self.i]
    }

    fn unquoted(&mut self) -> &'a str {
        let start = self.i;
        let mut end = start;
        while end < self.s.len() && !b"\n\r,}]#".contains(&self.s[end]) && !self.s[end..].starts_with(b"//") {
            end += 1;
        }
        let text = self.src[start..end].trim_end();
        self.i = start + text.len();
        text
    }

    fn number_word(&self, word: &str) -> Option<(Kind, Json, String)> {
        let lower = word.to_ascii_lowercase();
        let unsigned = lower.trim_start_matches(['-', '+']);
        if let Some(hex) = unsigned.strip_prefix("0x") {
            let n = i64::from_str_radix(hex, 16).ok()?;
            return Some((Kind::Int, Json::from(if lower.starts_with('-') { -n } else { n }), String::new()));
        }
        if !word.contains(['.', 'e', 'E']) {
            if let Ok(n) = word.parse::<i64>() {
                return Some((Kind::Int, Json::from(n), String::new()));
            }
        }
        if let Some(n) = number(word).and_then(serde_json::Number::from_f64) {
            return Some((Kind::Float, Json::Number(n), String::new()));
        }
        if self.dialect == Dialect::Snbt {
            let suffix = word.chars().last().filter(|c| "bBsSlLfFdD".contains(*c))?;
            let body = &word[..word.len() - 1];
            let (kind, value, _) = self.number_word(body)?;
            let kind = if "fFdD".contains(suffix) { Kind::Float } else { kind };
            let value = if kind == Kind::Float { serde_json::Number::from_f64(number(body)?).map(Json::Number)? } else { value };
            return Some((kind, value, suffix.to_string()));
        }
        None
    }

    fn scalar(&mut self, path: &[String], comment: Vec<String>) -> AppResult<bool> {
        let start = self.i;
        let mut affix = String::new();
        let (kind, value, quoted) = if self.s[self.i] == b'"' || self.s[self.i] == b'\'' {
            let text = self.string()?;
            (Kind::String, Json::from(text), true)
        } else if self.dialect == Dialect::Hocon {
            let text = self.unquoted();
            if text.is_empty() {
                return self.fail("a missing value");
            }
            if text == "null" {
                (Kind::Other, Json::Null, false)
            } else {
                let (kind, value) = infer(text);
                (kind, value, false)
            }
        } else {
            let word = self.word();
            if word.is_empty() {
                return self.fail("an unexpected character");
            }
            let unsigned = word.to_ascii_lowercase();
            let unsigned = unsigned.trim_start_matches(['-', '+']);
            if word == "true" || word == "false" {
                (Kind::Bool, Json::from(word == "true"), false)
            } else if word == "null" || unsigned == "infinity" || unsigned == "nan" {
                (Kind::Other, Json::Null, false)
            } else if let Some((kind, value, suffix)) = self.number_word(word) {
                affix = suffix;
                (kind, value, false)
            } else if self.dialect == Dialect::Snbt {
                (Kind::String, Json::from(word), false)
            } else {
                return self.fail("a bad value");
            }
        };
        if !path.is_empty() {
            self.nodes.push(Node { quoted, affix, ..Node::new(path.to_vec(), kind, value, (start, self.i), comment) });
        }
        Ok(true)
    }

    fn after_comma(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] == b' ' || self.s[self.i] == b'\t') {
            self.i += 1;
        }
        if self.s[self.i..].starts_with(b"//") || (self.hash_comments() && self.s.get(self.i) == Some(&b'#')) {
            self.i = self.line_end();
        }
    }

    fn separator(&mut self, close: u8) -> AppResult<()> {
        self.skip();
        match self.s.get(self.i) {
            Some(b',') => {
                self.i += 1;
                self.after_comma();
                Ok(())
            }
            Some(c) if *c == close => Ok(()),
            None if self.dialect == Dialect::Hocon && close == 0 => Ok(()),
            _ if self.dialect != Dialect::Json => Ok(()),
            _ => self.fail("a missing ','"),
        }
    }

    fn note(&mut self, key: &str, at: usize) -> Option<Vec<String>> {
        let label = match key {
            "comment" | "_comment" | "__comment" => "",
            _ => key.strip_prefix("//")?,
        };
        let text = match (&self.nodes.get(at)?.value, self.nodes.len() == at + 1) {
            (Json::String(s), true) => s.clone(),
            (Json::Bool(_) | Json::Number(_), true) if !label.is_empty() => self.nodes[at].value.to_string(),
            _ => return None,
        };
        let mut lines = self.nodes.pop()?.comment;
        for line in text.lines() {
            lines.push(if label.is_empty() { line.to_string() } else { format!("{label}: {line}") });
        }
        Some(lines)
    }

    fn members(&mut self, path: &[String], close: u8, section: Option<usize>) -> AppResult<()> {
        let mut noted = false;
        loop {
            self.skip();
            if self.i >= self.s.len() {
                return if close == 0 { Ok(()) } else { self.fail("an unclosed object") };
            }
            if self.s[self.i] == close {
                if let (true, Some(section)) = (noted, section) {
                    let lines = std::mem::take(&mut self.comments);
                    self.nodes[section].comment.extend(lines);
                }
                self.i += 1;
                return Ok(());
            }
            let key = if self.s[self.i] == b'"' || self.s[self.i] == b'\'' {
                self.string()?
            } else {
                let word = self.word();
                if word.is_empty() {
                    return self.fail("a missing key");
                }
                word.to_string()
            };
            let comment = std::mem::take(&mut self.comments);
            self.skip();
            match self.s.get(self.i) {
                Some(b':') => self.i += 1,
                Some(b'=') if self.dialect == Dialect::Hocon => self.i += 1,
                Some(b'{') if self.dialect == Dialect::Hocon => {}
                _ => return self.fail("a missing ':'"),
            }
            let mut at = path.to_vec();
            at.push(key.clone());
            let before = self.nodes.len();
            self.value(&at, comment)?;
            self.comments.clear();
            if let Some(lines) = self.note(&key, before) {
                self.comments = lines;
                noted = true;
            } else {
                if let (true, Some(section)) = (noted, section) {
                    if self.nodes[before].kind == Kind::Section {
                        let lines = std::mem::take(&mut self.nodes[before].comment);
                        self.nodes[section].comment.extend(lines);
                    }
                }
                noted = false;
            }
            self.separator(close)?;
        }
    }

    fn value(&mut self, path: &[String], comment: Vec<String>) -> AppResult<bool> {
        self.skip();
        if self.i >= self.s.len() {
            return self.fail("an unexpected end");
        }
        match self.s[self.i] {
            b'{' => {
                let section = (!path.is_empty()).then(|| {
                    self.nodes.push(Node::new(path.to_vec(), Kind::Section, Json::Null, (self.i, self.i), comment));
                    self.nodes.len() - 1
                });
                self.i += 1;
                self.comments.clear();
                self.members(path, b'}', section)?;
                Ok(false)
            }
            b'[' => {
                let start = self.i;
                let before = self.nodes.len();
                self.i += 1;
                let typed = self.dialect == Dialect::Snbt
                    && self.s.get(self.i).is_some_and(|c| b"BIL".contains(c))
                    && self.s.get(self.i + 1) == Some(&b';');
                if typed {
                    self.i += 2;
                }
                let mut scalars = true;
                let mut index = 0usize;
                loop {
                    self.skip();
                    if self.i >= self.s.len() {
                        return self.fail("an unclosed list");
                    }
                    if self.s[self.i] == b']' {
                        self.i += 1;
                        break;
                    }
                    let mut at = path.to_vec();
                    at.push(index.to_string());
                    let scalar = self.value(&at, Vec::new())?;
                    scalars &= scalar;
                    index += 1;
                    self.separator(b']')?;
                }
                if path.is_empty() {
                    return Ok(false);
                }
                if typed {
                    self.nodes.truncate(before);
                    self.nodes.push(Node::new(path.to_vec(), Kind::Other, Json::Null, (start, self.i), comment));
                } else if scalars {
                    let values = self.nodes.drain(before..).map(|n| n.value).collect();
                    self.nodes.push(Node::new(path.to_vec(), Kind::List, Json::Array(values), (start, self.i), comment));
                } else {
                    self.nodes.insert(before, Node::new(path.to_vec(), Kind::Section, Json::Null, (start, start), comment));
                }
                Ok(false)
            }
            _ => self.scalar(path, comment),
        }
    }
}

fn parse_json(src: &str, dialect: Dialect) -> AppResult<Vec<Node>> {
    let mut p = JsonParser { s: src.as_bytes(), src, i: 0, nodes: Vec::new(), comments: Vec::new(), dialect };
    p.skip();
    if p.i >= p.s.len() {
        return Ok(Vec::new());
    }
    if dialect == Dialect::Hocon && !matches!(p.s.get(p.i), Some(b'{') | Some(b'[')) {
        p.members(&[], 0, None)?;
    } else {
        p.value(&[], Vec::new())?;
    }
    p.skip();
    if p.i < p.s.len() {
        return p.fail("text after the end");
    }
    Ok(p.nodes)
}

fn strip_yaml_comment(text: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut previous = ' ';
    for (at, c) in text.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if (c == '"' || c == '\'') && previous.is_whitespace() => quote = Some(c),
            None if c == '#' && previous.is_whitespace() => return text[..at].trim_end(),
            None => {}
        }
        previous = c;
    }
    text.trim_end()
}

fn yaml_scalar(text: &str) -> (Kind, Json, bool) {
    if text.starts_with('"') {
        return match serde_json::from_str::<String>(text) {
            Ok(s) => (Kind::String, Json::from(s), true),
            Err(_) => (Kind::Other, Json::Null, false),
        };
    }
    if let Some(inner) = text.strip_prefix('\'').and_then(|t| t.strip_suffix('\'')) {
        return (Kind::String, Json::from(inner.replace("''", "'")), true);
    }
    match text {
        "true" | "True" | "TRUE" => (Kind::Bool, Json::from(true), false),
        "false" | "False" | "FALSE" => (Kind::Bool, Json::from(false), false),
        "" | "~" | "null" | "Null" | "NULL" => (Kind::Other, Json::Null, false),
        _ if text.starts_with(['[', '{', '&', '*', '!', '|', '>']) => (Kind::Other, Json::Null, false),
        _ => {
            let (kind, value) = infer(text);
            (kind, value, false)
        }
    }
}

fn yaml_key(line: &str) -> Option<(String, usize)> {
    let indent = line.len() - line.trim_start().len();
    let rest = &line[indent..];
    if let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') {
        let close = rest[1..].find(quote)? + 1;
        let after = rest[close + 1..].trim_start();
        let colon = rest.len() - after.len();
        return after.starts_with(':').then(|| (rest[1..close].to_string(), indent + colon + 1));
    }
    let at = rest.find(": ").or_else(|| rest.ends_with(':').then(|| rest.len() - 1))?;
    let key = rest[..at].trim();
    (!key.is_empty() && !key.starts_with(['-', '?', '#'])).then(|| (key.to_string(), indent + at + 1))
}

fn parse_yaml(src: &str) -> Vec<Node> {
    let lines = lines_with_offsets(src);
    let significant = |j: usize| {
        let t = lines[j].1.trim();
        !t.is_empty() && !t.starts_with('#')
    };
    let indent_of = |line: &str| line.len() - line.trim_start().len();
    let mut nodes = Vec::new();
    let mut comment: Vec<String> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let (offset, line) = lines[i];
        let t = line.trim();
        if t.is_empty() {
            comment.clear();
            i += 1;
            continue;
        }
        if t.starts_with('#') {
            comment.extend(comment_lines(t, &['#']));
            i += 1;
            continue;
        }
        let indent = indent_of(line);
        if t == "---" || t == "..." || t.starts_with('-') {
            comment.clear();
            i += 1;
            continue;
        }
        while stack.last().is_some_and(|(depth, _)| *depth >= indent) {
            stack.pop();
        }
        let Some((key, colon)) = yaml_key(line) else {
            comment.clear();
            i += 1;
            continue;
        };
        let mut path: Vec<String> = stack.iter().map(|(_, k)| k.clone()).collect();
        path.push(key.clone());
        let comment_now = std::mem::take(&mut comment);
        let raw = strip_yaml_comment(&line[colon..]);
        let value = raw.trim_start();
        if value.is_empty() {
            let next = (i + 1..lines.len()).find(|&j| significant(j));
            if let Some(j) = next {
                let next_line = lines[j].1;
                let next_indent = indent_of(next_line);
                let next_t = next_line.trim();
                if next_t.starts_with('-') && next_indent >= indent {
                    let mut items = Vec::new();
                    let mut scalars = true;
                    let mut last = j;
                    let mut k = j;
                    while k < lines.len() {
                        let l = lines[k].1;
                        let lt = l.trim();
                        if lt.is_empty() || lt.starts_with('#') {
                            k += 1;
                            continue;
                        }
                        let li = indent_of(l);
                        if li < next_indent || (li == next_indent && !lt.starts_with('-')) {
                            break;
                        }
                        if li == next_indent {
                            let item = strip_yaml_comment(lt[1..].trim_start());
                            let (kind, value, _) = yaml_scalar(item);
                            if item.is_empty() || yaml_key(&format!(" {item}")).is_some() || matches!(kind, Kind::Other) {
                                scalars = false;
                            }
                            items.push(value);
                        } else {
                            scalars = false;
                        }
                        last = k;
                        k += 1;
                    }
                    let span = (lines[j].0, lines[last].0 + lines[last].1.len());
                    let node = if scalars {
                        Node { affix: format!("{}- ", " ".repeat(next_indent)), ..Node::new(path, Kind::List, Json::Array(items), span, comment_now) }
                    } else {
                        Node::new(path, Kind::Other, Json::Null, span, comment_now)
                    };
                    nodes.push(node);
                    i = k;
                    continue;
                }
                if next_indent > indent && !next_t.starts_with(['[', '{', '|', '>']) {
                    nodes.push(Node::new(path, Kind::Section, Json::Null, (offset, offset), comment_now));
                    stack.push((indent, key));
                    i += 1;
                    continue;
                }
            }
            nodes.push(Node::new(path, Kind::Other, Json::Null, (offset, offset), comment_now));
            i += 1;
            continue;
        }
        let start = offset + line.len() - line[colon..].trim_start().len();
        let span = (start, start + value.len());
        if value.starts_with(['|', '>']) {
            nodes.push(Node::new(path, Kind::Other, Json::Null, span, comment_now));
            i += 1;
            while i < lines.len() && (!significant(i) || indent_of(lines[i].1) > indent) {
                i += 1;
            }
            continue;
        }
        let (kind, parsed, quoted) = yaml_scalar(value);
        nodes.push(Node { quoted, ..Node::new(path, kind, parsed, span, comment_now) });
        i += 1;
    }
    nodes
}

fn yaml_plain(s: &str) -> bool {
    !s.is_empty()
        && s == s.trim()
        && !s.contains(['\n', '\r', '\t'])
        && !s.starts_with(['-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`'])
        && !s.contains(": ")
        && !s.contains(" #")
        && !s.ends_with(':')
        && yaml_scalar(s).0 == Kind::String
}

fn yaml_text(value: &Json) -> Option<String> {
    Some(match value {
        Json::String(s) if yaml_plain(s) => s.clone(),
        Json::String(s) => serde_json::to_string(s).ok()?,
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => n.to_string(),
        _ => return None,
    })
}

fn is_forge_typed(line: &str) -> bool {
    let b = line.as_bytes();
    b.len() > 2 && b"BIDS".contains(&b[0]) && b[1] == b':'
}

fn infer(raw: &str) -> (Kind, Json) {
    let t = raw.trim();
    if t == "true" || t == "false" {
        (Kind::Bool, Json::from(t == "true"))
    } else if let Ok(n) = t.parse::<i64>() {
        (Kind::Int, Json::from(n))
    } else if let Some(n) = (t.contains('.') || t.contains(['e', 'E'])).then(|| number(t)).flatten().and_then(serde_json::Number::from_f64) {
        (Kind::Float, Json::Number(n))
    } else {
        (Kind::String, Json::from(t))
    }
}

fn lines_with_offsets(src: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut at = 0;
    for line in src.split_inclusive('\n') {
        out.push((at, line.trim_end_matches(['\n', '\r'])));
        at += line.len();
    }
    out
}

fn value_span(offset: usize, line: &str, from: usize) -> (usize, usize) {
    let rest = &line[from..];
    let start = from + (rest.len() - rest.trim_start().len());
    let end = line.trim_end().len().max(start);
    (offset + start, offset + end)
}

fn parse_key_values(src: &str, format: Format) -> Vec<Node> {
    let markers: &[char] = match format {
        Format::Properties => &['#', '!'],
        Format::Options => &[],
        _ => &['#', ';'],
    };
    let sections = matches!(format, Format::Ini | Format::Cfg);
    let mut nodes = Vec::new();
    let mut comment: Vec<String> = Vec::new();
    let mut section: Option<String> = None;
    for (offset, line) in lines_with_offsets(src) {
        let t = line.trim();
        if t.is_empty() {
            comment.clear();
            continue;
        }
        if t.chars().next().is_some_and(|c| markers.contains(&c)) {
            comment.extend(comment_lines(t, markers));
            continue;
        }
        if sections && t.starts_with('[') && t.ends_with(']') {
            let name = t[1..t.len() - 1].trim().to_string();
            nodes.push(Node::new(vec![name.clone()], Kind::Section, Json::Null, (offset, offset), std::mem::take(&mut comment)));
            section = Some(name);
            continue;
        }
        let sep = match format {
            Format::Options => line.find(':'),
            Format::Properties => line.find(['=', ':']),
            _ => line.find('='),
        };
        let Some(sep) = sep else {
            comment.clear();
            continue;
        };
        let key = line[..sep].trim().to_string();
        if key.is_empty() {
            continue;
        }
        let span = value_span(offset, line, sep + 1);
        let raw = &src[span.0..span.1];
        let (kind, value, quoted) = if format == Format::Options && raw.starts_with('"') {
            match serde_json::from_str::<String>(raw) {
                Ok(s) => (Kind::String, Json::from(s), true),
                Err(_) => (Kind::Other, Json::Null, false),
            }
        } else if format == Format::Options && (raw.starts_with('[') || raw.starts_with('{')) {
            (Kind::Other, Json::Null, false)
        } else {
            let (kind, value) = infer(raw);
            (kind, value, false)
        };
        let mut path: Vec<String> = section.iter().cloned().collect();
        path.push(key);
        nodes.push(Node { quoted, ..Node::new(path, kind, value, span, std::mem::take(&mut comment)) });
    }
    nodes
}

fn parse_forge_cfg(src: &str) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut comment: Vec<String> = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut in_list = false;
    for (offset, line) in lines_with_offsets(src) {
        let t = line.trim();
        if in_list {
            in_list = t != ">";
            continue;
        }
        if t.is_empty() {
            comment.clear();
            continue;
        }
        if t.starts_with('#') {
            comment.extend(comment_lines(t, &['#']));
            continue;
        }
        if t == "}" {
            path.pop();
            comment.clear();
            continue;
        }
        if let Some(name) = t.strip_suffix('{') {
            path.push(name.trim().trim_matches('"').to_string());
            nodes.push(Node::new(path.clone(), Kind::Section, Json::Null, (offset, offset), std::mem::take(&mut comment)));
            continue;
        }
        if !is_forge_typed(t) {
            comment.clear();
            continue;
        }
        let body = &t[2..];
        let (name, after) = if let Some(quoted) = body.strip_prefix('"') {
            match quoted.find('"') {
                Some(end) => (quoted[..end].to_string(), &quoted[end + 1..]),
                None => continue,
            }
        } else {
            let end = body.find(['=', '<']).unwrap_or(body.len());
            (body[..end].trim().to_string(), &body[end..])
        };
        let after = after.trim_start();
        let mut at = path.clone();
        at.push(name);
        if after.starts_with('<') {
            in_list = !after.contains('>');
            nodes.push(Node::new(at, Kind::Other, Json::Null, (offset, offset), std::mem::take(&mut comment)));
            continue;
        }
        if !after.starts_with('=') {
            comment.clear();
            continue;
        }
        let at_eq = after.as_ptr() as usize - line.as_ptr() as usize;
        let span = value_span(offset, line, at_eq + 1);
        let raw = &src[span.0..span.1];
        let kind = match t.as_bytes()[0] {
            b'B' => Kind::Bool,
            b'I' => Kind::Int,
            b'D' => Kind::Float,
            _ => Kind::String,
        };
        let value = typed(kind, raw).unwrap_or(Json::Null);
        let kind = if value.is_null() { Kind::Other } else { kind };
        nodes.push(Node::new(at, kind, value, span, std::mem::take(&mut comment)));
    }
    nodes
}

fn unique(mut nodes: Vec<Node>) -> Vec<Node> {
    let mut seen: std::collections::HashMap<Vec<String>, usize> = std::collections::HashMap::new();
    for node in &mut nodes {
        let count = seen.entry(node.path.clone()).or_insert(0);
        *count += 1;
        if *count > 1 {
            if let Some(last) = node.path.last_mut() {
                *last = format!("{last}#{count}");
            }
        }
    }
    nodes
}

fn nodes_of(src: &str, format: Format) -> AppResult<Vec<Node>> {
    Ok(unique(match format {
        Format::Toml => parse_toml(src)?,
        Format::Json | Format::Json5 | Format::Jsonc => parse_json(src, Dialect::Json)?,
        Format::Snbt => parse_json(src, Dialect::Snbt)?,
        Format::Hocon => parse_json(src, Dialect::Hocon)?,
        Format::Yaml => parse_yaml(src),
        Format::Cfg if src.lines().any(|l| is_forge_typed(l.trim_start())) => parse_forge_cfg(src),
        _ => parse_key_values(src, format),
    }))
}

fn float_text(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{f:.1}")
    } else {
        f.to_string()
    }
}

fn literal(node: &Node, format: Format, value: &Json, newline: &str) -> Option<String> {
    let json = matches!(format, Format::Json | Format::Json5 | Format::Jsonc | Format::Toml | Format::Snbt | Format::Hocon);
    if format == Format::Yaml {
        return Some(match (node.kind, value) {
            (Kind::String, Json::String(s)) if node.quoted || !yaml_plain(s) => serde_json::to_string(s).ok()?,
            (Kind::List, Json::Array(items)) if items.is_empty() => format!("{}[]", " ".repeat(node.affix.len())),
            (Kind::List, Json::Array(items)) => {
                let lines: Option<Vec<String>> = items.iter().map(|i| yaml_text(i).map(|t| format!("{}{t}", node.affix))).collect();
                lines?.join(newline)
            }
            (Kind::Float, Json::Number(n)) => float_text(n.as_f64()?),
            (Kind::Int | Kind::Bool | Kind::String, _) => yaml_text(value).filter(|_| matches!((node.kind, value), (Kind::Int, Json::Number(n)) if n.is_i64()) || matches!((node.kind, value), (Kind::Bool, Json::Bool(_)) | (Kind::String, Json::String(_))))?,
            _ => return None,
        });
    }
    Some(match (node.kind, value) {
        (Kind::Bool, Json::Bool(b)) => b.to_string(),
        (Kind::Int, Json::Number(n)) => format!("{}{}", n.as_i64()?, node.affix),
        (Kind::Float, Json::Number(n)) => format!("{}{}", float_text(n.as_f64()?), node.affix),
        (Kind::String, Json::String(s)) if json || node.quoted => serde_json::to_string(s).ok()?,
        (Kind::String, Json::String(s)) if !s.contains(['\n', '\r']) => s.clone(),
        (Kind::List, Json::Array(items)) if json && items.iter().all(|i| !i.is_array() && !i.is_object()) => {
            let parts: Option<Vec<String>> = items.iter().map(|i| serde_json::to_string(i).ok()).collect();
            format!("[{}]", parts?.join(", "))
        }
        _ => return None,
    })
}

fn splice_write(src: &str, format: Format, changes: &[Change]) -> AppResult<String> {
    let nodes = nodes_of(src, format)?;
    let newline = if src.contains("\r\n") { "\r\n" } else { "\n" };
    let mut edits = Vec::new();
    for change in changes {
        let node = nodes
            .iter()
            .find(|n| n.path == change.path && n.kind != Kind::Section)
            .ok_or_else(|| missing(&change.path))?;
        if node.value == change.value {
            continue;
        }
        let text = literal(node, format, &change.value, newline).ok_or_else(|| mismatch(&change.path))?;
        edits.push((node.span, text));
    }
    edits.sort_by(|a, b| b.0 .0.cmp(&a.0 .0));
    let mut out = src.to_string();
    for ((start, end), text) in edits {
        out.replace_range(start..end, &text);
    }
    nodes_of(&out, format)?;
    Ok(out)
}

fn missing(path: &[String]) -> AppError {
    AppError::not_found(format!("{} is no longer in the file", path.join(".")))
}

fn mismatch(path: &[String]) -> AppError {
    AppError::invalid(format!("{} cannot take that value", path.join(".")))
}

pub fn read_doc(src: &str, format: Format) -> AppResult<Vec<Entry>> {
    Ok(nodes_of(src, format)?.into_iter().map(|n| entry(n.path, n.kind, n.value, &n.comment)).collect())
}

pub fn write_doc(src: &str, format: Format, changes: &[Change]) -> AppResult<String> {
    splice_write(src, format, changes)
}

fn resolve(id: &str, rel: &str) -> AppResult<(PathBuf, Format)> {
    if !allowed_rel(rel) {
        return Err(AppError::invalid("not a config file"));
    }
    let format = format_of(rel).ok_or_else(|| AppError::invalid("not a config file"))?;
    Ok((paths::instance_game_dir(id).join(rel), format))
}

fn read_text(path: &Path) -> AppResult<(String, String)> {
    if std::fs::metadata(path)?.len() > MAX_BYTES {
        return Err(AppError::invalid("this file is too large to edit here"));
    }
    let text = String::from_utf8(std::fs::read(path)?).map_err(|_| AppError::invalid("this file is not text"))?;
    Ok(match text.strip_prefix('\u{feff}') {
        Some(body) => ("\u{feff}".to_string(), body.to_string()),
        None => (String::new(), text),
    })
}

#[tauri::command]
pub async fn list_configs(id: String) -> AppResult<Vec<ConfigFile>> {
    crate::blocking(move || Ok(list(&paths::instance_game_dir(&id)))).await
}

#[tauri::command]
pub async fn read_config(id: String, rel: String) -> AppResult<ConfigDoc> {
    crate::blocking(move || {
        let (path, format) = resolve(&id, &rel)?;
        let (_, text) = read_text(&path)?;
        Ok(ConfigDoc { format, entries: read_doc(&text, format)? })
    })
    .await
}

#[tauri::command]
pub async fn write_config(state: State<'_, AppState>, id: String, rel: String, changes: Vec<Change>) -> AppResult<()> {
    if state.running.lock().map_err(|e| e.to_string())?.contains(&id) {
        return Err(AppError::busy("Close the game before changing its settings."));
    }
    crate::blocking(move || {
        let (path, format) = resolve(&id, &rel)?;
        let (bom, text) = read_text(&path)?;
        let updated = write_doc(&text, format, &changes)?;
        let backup = paths::instance_dir(&id).join("config-backups").join(&rel);
        if let Some(parent) = backup.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&path, &backup)?;
        std::fs::write(&path, format!("{bom}{updated}"))?;
        Ok(())
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(path: &[&str], value: Json) -> Change {
        Change { path: path.iter().map(|p| p.to_string()).collect(), value }
    }

    fn find<'a>(entries: &'a [Entry], path: &[&str]) -> &'a Entry {
        entries.iter().find(|e| e.path == path).unwrap_or_else(|| panic!("no {path:?}"))
    }

    const NEOFORGE: &str = "#Everything else\n[other]\n\t#If true, cats drop things.\n\tdoCatDrops = true\n\t#The amount of ticks it takes for a worm to die.\n\t# Default: 0\n\t# Range: 0 ~ 10000000\n\twormDeathTime = 0\n\t#Mode\n\t#Allowed Values: FAST, FANCY\n\tmode = \"FAST\"\n\tspeed = 1.5 # fast\n\titems = [\"a\", \"b\"]\n";

    #[test]
    fn a_neoforge_toml_turns_into_a_form() {
        let entries = read_doc(NEOFORGE, Format::Toml).unwrap();
        assert_eq!(find(&entries, &["other"]).kind, Kind::Section);
        assert_eq!(find(&entries, &["other"]).comment.as_deref(), Some("Everything else"));
        let cats = find(&entries, &["other", "doCatDrops"]);
        assert_eq!((cats.kind, &cats.value), (Kind::Bool, &json!(true)));
        assert_eq!(cats.comment.as_deref(), Some("If true, cats drop things."));
        let worm = find(&entries, &["other", "wormDeathTime"]);
        assert_eq!((worm.min, worm.max, &worm.default), (Some(0.0), Some(10_000_000.0), &Some(json!(0))));
        assert_eq!(worm.comment.as_deref(), Some("The amount of ticks it takes for a worm to die."));
        assert_eq!(find(&entries, &["other", "mode"]).options, vec!["FAST", "FANCY"]);
        assert_eq!(find(&entries, &["other", "items"]).value, json!(["a", "b"]));
    }

    #[test]
    fn saving_a_toml_changes_only_the_value() {
        let out = write_doc(
            NEOFORGE,
            Format::Toml,
            &[change(&["other", "wormDeathTime"], json!(40)), change(&["other", "speed"], json!(2.0)), change(&["other", "mode"], json!("FANCY"))],
        )
        .unwrap();
        assert!(out.contains("\t# Range: 0 ~ 10000000\n\twormDeathTime = 40\n"));
        assert!(out.contains("speed = 2.0 # fast"));
        assert!(out.contains("\t#Allowed Values: FAST, FANCY\n\tmode = \"FANCY\"\n"));
        let changed: Vec<(&str, &str)> = NEOFORGE.lines().zip(out.lines()).filter(|(a, b)| a != b).collect();
        assert_eq!(changed.len(), 3);
        assert_eq!(out.lines().count(), NEOFORGE.lines().count());
    }

    #[test]
    fn a_trailing_comment_stays_with_its_own_line() {
        let src = "{\n  \"a\": 1, // about a\n  \"b\": 2\n}";
        let entries = read_doc(src, Format::Jsonc).unwrap();
        assert_eq!(find(&entries, &["b"]).comment, None);
    }

    #[test]
    fn a_value_of_the_wrong_type_is_refused() {
        assert!(write_doc(NEOFORGE, Format::Toml, &[change(&["other", "doCatDrops"], json!("yes"))]).is_err());
        assert!(write_doc(NEOFORGE, Format::Toml, &[change(&["other", "nope"], json!(true))]).is_err());
    }

    const JSON5: &str = "{\n\t// Whether dark mode should be enabled.\n\t\"darkMode\": false,\n\tsize: 12,\n\t'name': 'x',\n\t\"window\": { \"x\": 0, \"scale\": 1.0 },\n\t\"tags\": [\"a\", \"b\"],\n\t\"rules\": [{ \"on\": true }],\n}\n";

    #[test]
    fn json5_keeps_its_comments_and_quotes() {
        let entries = read_doc(JSON5, Format::Json5).unwrap();
        assert_eq!(find(&entries, &["darkMode"]).comment.as_deref(), Some("Whether dark mode should be enabled."));
        assert_eq!(find(&entries, &["size"]).kind, Kind::Int);
        assert_eq!(find(&entries, &["window"]).kind, Kind::Section);
        assert_eq!(find(&entries, &["window", "scale"]).kind, Kind::Float);
        assert_eq!(find(&entries, &["tags"]).value, json!(["a", "b"]));
        assert_eq!(find(&entries, &["rules", "0", "on"]).value, json!(true));

        let out = write_doc(
            JSON5,
            Format::Json5,
            &[change(&["darkMode"], json!(true)), change(&["window", "scale"], json!(2)), change(&["name"], json!("y \"q\"")), change(&["tags"], json!(["c"]))],
        )
        .unwrap();
        assert!(out.contains("// Whether dark mode should be enabled.\n\t\"darkMode\": true,"));
        assert!(out.contains("\"scale\": 2.0 }"));
        assert!(out.contains("'name': \"y \\\"q\\\"\","));
        assert!(out.contains("\"tags\": [\"c\"],"));
    }

    #[test]
    fn plain_json_round_trips_untouched_parts() {
        let src = "{\n  \"enableBorderlessFullscreen\": true,\n  \"customWindowDimensions\": {\n    \"enabled\": false,\n    \"x\": 0\n  },\n  \"forceWindowMonitor\": -1\n}";
        let out = write_doc(src, Format::Json, &[change(&["customWindowDimensions", "x"], json!(-5))]).unwrap();
        assert_eq!(out, src.replace("\"x\": 0", "\"x\": -5"));
        assert!(read_doc("{ \"a\": ", Format::Json).is_err());
    }

    #[test]
    fn forge_cfg_reads_types_and_ranges() {
        let src = "# File Specification\n\ngeneral {\n    # The level of silk touch needed.\n    # Synced.\n    # Default: 1; Range: [-1 ~ 127]\n    I:\"Spawner Silk Level\"=1\n\n    # Chance [range: 0.0 ~ 1.0, default: 0.005]\n    D:chance=0.005\n    B:enabled=true\n    S:list <\n        a\n     >\n}\n";
        let entries = read_doc(src, Format::Cfg).unwrap();
        let silk = find(&entries, &["general", "Spawner Silk Level"]);
        assert_eq!((silk.kind, silk.min, silk.max, &silk.default), (Kind::Int, Some(-1.0), Some(127.0), &Some(json!(1))));
        assert_eq!(silk.comment.as_deref(), Some("The level of silk touch needed.\nSynced."));
        let chance = find(&entries, &["general", "chance"]);
        assert_eq!((chance.min, chance.max, chance.comment.as_deref()), (Some(0.0), Some(1.0), Some("Chance")));
        assert_eq!(find(&entries, &["general", "list"]).kind, Kind::Other);
        let out = write_doc(src, Format::Cfg, &[change(&["general", "Spawner Silk Level"], json!(5)), change(&["general", "enabled"], json!(false))]).unwrap();
        assert!(out.contains("I:\"Spawner Silk Level\"=5\n"));
        assert!(out.contains("B:enabled=false\n"));
    }

    #[test]
    fn properties_ini_and_options_are_key_value_lines() {
        let props = "# comment\r\noverlay.color=0\r\ndisplay.ro3=true\r\nname = hello world\r\n";
        let entries = read_doc(props, Format::Properties).unwrap();
        assert_eq!(find(&entries, &["overlay.color"]).comment.as_deref(), Some("comment"));
        assert_eq!(find(&entries, &["name"]).value, json!("hello world"));
        let out = write_doc(props, Format::Properties, &[change(&["display.ro3"], json!(false)), change(&["name"], json!("bye"))]).unwrap();
        assert_eq!(out, "# comment\r\noverlay.color=0\r\ndisplay.ro3=false\r\nname = bye\r\n");

        let ini = "[Window][Debug]\nPos=60,60\nCollapsed=0\n";
        assert_eq!(find(&read_doc(ini, Format::Ini).unwrap(), &["Window][Debug", "Collapsed"]).kind, Kind::Int);

        let options = "version:4671\nao:true\nlang:en_us\nresourcePacks:[\"vanilla\"]\nlastServer:\"play.example.net\"\nfov:0.25\n";
        let entries = read_doc(options, Format::Options).unwrap();
        assert_eq!(find(&entries, &["resourcePacks"]).kind, Kind::Other);
        assert_eq!(find(&entries, &["lastServer"]).value, json!("play.example.net"));
        let out = write_doc(options, Format::Options, &[change(&["fov"], json!(0.5)), change(&["lastServer"], json!("x.y")), change(&["lang"], json!("pl_pl"))]).unwrap();
        assert_eq!(out, "version:4671\nao:true\nlang:pl_pl\nresourcePacks:[\"vanilla\"]\nlastServer:\"x.y\"\nfov:0.5\n");
        assert!(write_doc(options, Format::Options, &[change(&["lang"], json!("a\nb"))]).is_err());
    }

    const SNBT: &str = "# FTB Essentials config file\n\n{\n\t# If true, registers an alias\n\t# Default: false\n\tregister_alias: false\n\t\n\t# Admin commands\n\tadmin: {\n\t\t# Default: 500\n\t\t# Range: 0 ~ 10000\n\t\ttimer: 500\n\t\tscale: 1.0d\n\t\tbig: 10L\n\t\tname: \"x\"\n\t\tids: [I; 1, 2]\n\t\ttags: [\"a\", \"b\"]\n\t}\n}\n";

    #[test]
    fn ftb_snbt_keeps_its_number_types() {
        let entries = read_doc(SNBT, Format::Snbt).unwrap();
        assert_eq!(find(&entries, &["register_alias"]).comment.as_deref(), Some("If true, registers an alias"));
        let timer = find(&entries, &["admin", "timer"]);
        assert_eq!((timer.kind, timer.min, timer.max), (Kind::Int, Some(0.0), Some(10000.0)));
        assert_eq!(find(&entries, &["admin", "scale"]).kind, Kind::Float);
        assert_eq!(find(&entries, &["admin", "big"]).value, json!(10));
        assert_eq!(find(&entries, &["admin", "ids"]).kind, Kind::Other);
        let out = write_doc(
            SNBT,
            Format::Snbt,
            &[change(&["admin", "scale"], json!(0.5)), change(&["admin", "big"], json!(20)), change(&["admin", "tags"], json!(["c"])), change(&["register_alias"], json!(true))],
        )
        .unwrap();
        assert!(out.contains("\t\tscale: 0.5d\n\t\tbig: 20L\n"));
        assert!(out.contains("\t\ttags: [\"c\"]\n"));
        assert!(out.contains("\tregister_alias: true\n"));
    }

    #[test]
    fn hocon_with_and_without_braces() {
        let bobby = "# Delete regions after X days.\n# \n# Set to -1 to disable.\ndelete-unused-regions-after-days=-1\nenabled=true\ntitle = Hello world # note\nnested {\n    depth = 2\n}\n";
        let entries = read_doc(bobby, Format::Hocon).unwrap();
        assert_eq!(find(&entries, &["delete-unused-regions-after-days"]).value, json!(-1));
        assert_eq!(find(&entries, &["title"]).value, json!("Hello world"));
        assert_eq!(find(&entries, &["nested", "depth"]).value, json!(2));
        let out = write_doc(bobby, Format::Hocon, &[change(&["title"], json!("Bye")), change(&["nested", "depth"], json!(3))]).unwrap();
        assert!(out.contains("title = \"Bye\" # note\n"));
        assert!(out.contains("    depth = 3\n"));

        let respack = "{\n    debugCommands: false,\n    // Logs some information. (requires restart)\n    debugLogs: false,\n    ioLogs: false\n}";
        let entries = read_doc(respack, Format::Hocon).unwrap();
        assert_eq!(find(&entries, &["debugLogs"]).comment.as_deref(), Some("Logs some information. (requires restart)"));
    }

    const YAML: &str = "# The main file.\n\n# Toggles.\nfeatures:\n  # Prevents lag.\n  prevent-moving: false\n  autosave-interval-seconds: 300 # every 5 min\n  name: 'it''s'\n  lobotomize:\n    enabled: false\n# A list of ids.\nblacklist:\n- ars_nouveau:animated_block\n- \"quoted: item\"\nempty:\nscale: 1.5\n";

    #[test]
    fn yaml_reads_nesting_lists_and_comments() {
        let entries = read_doc(YAML, Format::Yaml).unwrap();
        assert_eq!(find(&entries, &["features"]).kind, Kind::Section);
        assert_eq!(find(&entries, &["features", "prevent-moving"]).comment.as_deref(), Some("Prevents lag."));
        assert_eq!(find(&entries, &["features", "autosave-interval-seconds"]).value, json!(300));
        assert_eq!(find(&entries, &["features", "name"]).value, json!("it's"));
        assert_eq!(find(&entries, &["features", "lobotomize", "enabled"]).kind, Kind::Bool);
        assert_eq!(find(&entries, &["blacklist"]).value, json!(["ars_nouveau:animated_block", "quoted: item"]));
        assert_eq!(find(&entries, &["empty"]).kind, Kind::Other);
        assert_eq!(find(&entries, &["scale"]).kind, Kind::Float);

        let out = write_doc(
            YAML,
            Format::Yaml,
            &[
                change(&["features", "autosave-interval-seconds"], json!(60)),
                change(&["features", "name"], json!("x")),
                change(&["blacklist"], json!(["a:b", "has # hash", "c"])),
                change(&["features", "lobotomize", "enabled"], json!(true)),
            ],
        )
        .unwrap();
        assert!(out.contains("  autosave-interval-seconds: 60 # every 5 min\n"));
        assert!(out.contains("  name: \"x\"\n"));
        assert!(out.contains("blacklist:\n- a:b\n- \"has # hash\"\n- c\nempty:\n"));
        assert!(out.contains("    enabled: true\n"));
        let emptied = write_doc(YAML, Format::Yaml, &[change(&["blacklist"], json!([]))]).unwrap();
        assert!(emptied.contains("blacklist:\n  []\nempty:"));
    }

    #[test]
    fn quest_data_is_not_a_config() {
        assert!(!allowed_rel("config/ftbquests/quests/chapters/intro.snbt"));
        assert!(allowed_rel("config/ftbchunks-client.snbt"));
        assert!(allowed_rel("config/servercore/config.yml"));
        assert!(allowed_rel("config/bobby.conf"));
    }

    #[test]
    fn comment_keys_in_json_become_comments() {
        let attributefix = "{\n  \"modify_range\": {\n    \"//\": \"Determines if the range should be modified.\",\n    \"//default\": false,\n    \"value\": false\n  },\n  \"max\": {\n    \"//\": \"The highest possible value.\",\n    \"//default\": 1024.0,\n    \"//range\": \">=0\",\n    \"value\": 1024.0\n  }\n}";
        let entries = read_doc(attributefix, Format::Json).unwrap();
        assert!(entries.iter().all(|e| !e.path.last().unwrap().starts_with("//")));
        let modify = find(&entries, &["modify_range", "value"]);
        assert_eq!(modify.comment.as_deref(), Some("Determines if the range should be modified."));
        assert_eq!(modify.default, Some(json!(false)));
        let max = find(&entries, &["max", "value"]);
        assert_eq!((max.default.clone(), max.min), (Some(json!(1024.0)), Some(0.0)));
        let out = write_doc(attributefix, Format::Json, &[change(&["max", "value"], json!(2048.0))]).unwrap();
        assert_eq!(out, attributefix.replace("\"value\": 1024.0", "\"value\": 2048.0"));

        let railways = "{\n  \"misc\": {\n    \"comment\": \"Miscellaneous settings\",\n    \"strictCoupler\": {\n      \"comment\": \"Allowed Values: DEFAULT, DISABLE\",\n      \"value\": \"DEFAULT\"\n    }\n  },\n  \"comment\": 5\n}";
        let entries = read_doc(railways, Format::Json).unwrap();
        assert_eq!(find(&entries, &["misc"]).comment.as_deref(), Some("Miscellaneous settings"));
        assert_eq!(find(&entries, &["misc", "strictCoupler"]).comment, None);
        assert_eq!(find(&entries, &["misc", "strictCoupler", "value"]).options, vec!["DEFAULT", "DISABLE"]);
        assert_eq!(find(&entries, &["comment"]).value, json!(5));
    }

    #[test]
    fn only_config_places_can_be_opened() {
        for ok in ["options.txt", "config/c2me.toml", "config/sub/x.json5", "defaultconfigs/a.toml", "saves/World/serverconfig/a-server.toml"] {
            assert!(allowed_rel(ok), "{ok}");
        }
        for bad in ["../x.toml", "config/../../x.toml", "mods/a.toml", "config/a.bak", "C:/x.toml", "config\\a.toml", "saves/World/data/a.json", "/config/a.toml", "config//a.toml"] {
            assert!(!allowed_rel(bad), "{bad}");
        }
    }

    #[test]
    fn listing_finds_configs_and_skips_the_rest() {
        let root = std::env::temp_dir().join(format!("spectra-configs-{}", uuid::Uuid::new_v4()));
        let game = root.join("minecraft");
        for (rel, body) in [
            ("options.txt", "ao:true\n"),
            ("config/a.toml", "x = 1\n"),
            ("config/sub/b.json5", "{}"),
            ("config/c.toml.bak", ""),
            ("config/.hidden.json", "{}"),
            ("config/pic.png", ""),
            ("saves/W/serverconfig/s.toml", "y = 2\n"),
        ] {
            let path = game.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        let rels: Vec<String> = list(&game).into_iter().map(|f| f.rel).collect();
        assert_eq!(rels, vec!["options.txt", "config/a.toml", "config/sub/b.json5", "saves/W/serverconfig/s.toml"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}


#[cfg(test)]
mod real_files {
    use super::*;

    fn mutate(entry: &Entry, format: Format) -> Option<Json> {
        let line_format = !matches!(format, Format::Toml | Format::Json | Format::Json5 | Format::Jsonc | Format::Snbt | Format::Hocon | Format::Yaml);
        Some(match (&entry.kind, &entry.value) {
            (Kind::Bool, Json::Bool(b)) => json!(!b),
            (Kind::Int, Json::Number(n)) => json!(n.as_i64()?.wrapping_add(1)),
            (Kind::Float, Json::Number(n)) => json!(n.as_f64()? + 0.5),
            (Kind::String, Json::String(t)) if line_format => json!(format!("{t}x")),
            (Kind::String, Json::String(t)) => json!(format!("{t} \"q\" \\ x")),
            (Kind::List, Json::Array(items)) => {
                let mut items = items.clone();
                items.push(items.first().cloned().unwrap_or(json!("x")));
                Json::Array(items)
            }
            _ => return None,
        })
    }

    #[test]
    #[ignore]
    fn every_real_config_survives_real_edits() {
        let root = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("SpectraLauncher/instances");
        let mut stats: std::collections::BTreeMap<String, (u32, u32, u32, u32)> = Default::default();
        let mut samples = Vec::new();
        for inst in std::fs::read_dir(&root).unwrap().flatten() {
            for file in list(&inst.path().join("minecraft")) {
                let stat = stats.entry(format!("{:?}", file.format)).or_default();
                stat.0 += 1;
                let Ok((_, text)) = read_text(std::path::Path::new(&file.path)) else { stat.1 += 1; continue };
                let entries = match read_doc(&text, file.format) {
                    Ok(entries) => entries,
                    Err(e) => {
                        stat.1 += 1;
                        samples.push(format!("{} :: unreadable: {}", file.path, e.message));
                        continue;
                    }
                };
                for target in entries.iter().filter(|e| !matches!(e.kind, Kind::Section | Kind::Other)).take(6) {
                    let Some(new) = mutate(target, file.format) else { continue };
                    stat.2 += 1;
                    let problem = match write_doc(&text, file.format, &[Change { path: target.path.clone(), value: new.clone() }]) {
                        Err(e) => Some(format!("write failed: {}", e.message)),
                        Ok(out) => match read_doc(&out, file.format) {
                            Err(e) => Some(format!("unreadable after write: {}", e.message)),
                            Ok(after) => {
                                let changed = after.iter().find(|e| e.path == target.path);
                                let others_same = entries.iter().filter(|e| e.path != target.path).all(|e| after.iter().any(|a| a.path == e.path && a.value == e.value));
                                let floats_close = matches!((changed.map(|c| &c.value), &new), (Some(Json::Number(a)), Json::Number(b)) if (a.as_f64().unwrap_or(0.0) - b.as_f64().unwrap_or(1.0)).abs() < 1e-9);
                                if changed.map(|c| &c.value) != Some(&new) && !floats_close {
                                    Some(format!("value is {:?}, wanted {new}", changed.map(|c| &c.value)))
                                } else if !others_same {
                                    Some("another value changed".into())
                                } else if target.kind != Kind::List && out.lines().count() != text.lines().count() {
                                    Some("line count changed".into())
                                } else {
                                    None
                                }
                            }
                        },
                    };
                    if let Some(problem) = problem {
                        stat.3 += 1;
                        if samples.len() < 30 {
                            samples.push(format!("{} :: {} :: {problem}", file.path, target.path.join(".")));
                        }
                    }
                }
            }
        }
        for (k, (n, unreadable, edits, failed)) in &stats {
            println!("{k}: files {n}, unreadable {unreadable}, edits tried {edits}, edits failed {failed}");
        }
        for s in samples {
            println!("{s}");
        }
    }
}
