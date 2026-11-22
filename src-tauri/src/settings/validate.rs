//! Finds problems in the user's `config.toml` and fixes them.
//!
//! Three passes, all on the raw TOML table of the user file (so a fix
//! edits exactly what the user wrote and nothing else):
//!
//! 1. [`migrate`] applies the declarative rules in `settings::rules`
//!    (renamed keys, retired keys, replaced provider ids). These are
//!    certain, so they run at load without asking.
//! 2. [`check_schema`] walks the file against the JSON Schema of
//!    [`Settings`](super::Settings): unknown keys, wrong types, invalid
//!    choices and out-of-range numbers (ranges come from `#[schemars]`
//!    attributes on the schema). A new settings field is covered as soon
//!    as it exists.
//! 3. [`check_providers`] checks every task chain against the providers
//!    actually registered (built-in and plugins), so an id the running
//!    build does not know is reported instead of silently failing.
//!
//! Passes 2 and 3 yield [`Issue`]s with a proposed [`Fix`]; the user
//! applies them from the home screen (`apply_config_fixes`).

use std::collections::{BTreeMap, HashMap};

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value as Json;
use toml::Value;

use super::rules::Rules;
use crate::providers::Capability;

/* --------------------------------- paths --------------------------------- */

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    Key(String),
    Index(usize),
}

pub type Path = Vec<Seg>;

fn key(k: &str) -> Seg {
    Seg::Key(k.to_string())
}

/// `a.b` paths from the rule table (keys there never contain dots).
pub fn dotted(p: &str) -> Path {
    p.split('.').map(key).collect()
}

/// `providers.configs."llamacpp.assistant"`, `fallback_on[1]`.
pub fn display(path: &[Seg]) -> String {
    let mut out = String::new();
    for seg in path {
        match seg {
            Seg::Key(k) => {
                if !out.is_empty() {
                    out.push('.');
                }
                if k.is_empty() || k.contains(['.', ' ', '"']) {
                    out.push_str(&format!("{k:?}"));
                } else {
                    out.push_str(k);
                }
            }
            Seg::Index(i) => out.push_str(&format!("[{i}]")),
        }
    }
    out
}

fn child<'a>(v: &'a Value, seg: &Seg) -> Option<&'a Value> {
    match (v, seg) {
        (Value::Table(t), Seg::Key(k)) => t.get(k),
        (Value::Array(a), Seg::Index(i)) => a.get(*i),
        _ => None,
    }
}

fn child_mut<'a>(v: &'a mut Value, seg: &Seg) -> Option<&'a mut Value> {
    match (v, seg) {
        (Value::Table(t), Seg::Key(k)) => t.get_mut(k),
        (Value::Array(a), Seg::Index(i)) => a.get_mut(*i),
        _ => None,
    }
}

pub fn get<'a>(t: &'a toml::Table, path: &[Seg]) -> Option<&'a Value> {
    let (Seg::Key(first), rest) = path.split_first()? else {
        return None;
    };
    rest.iter().try_fold(t.get(first)?, child)
}

fn get_mut<'a>(t: &'a mut toml::Table, path: &[Seg]) -> Option<&'a mut Value> {
    let (Seg::Key(first), rest) = path.split_first()? else {
        return None;
    };
    rest.iter()
        .try_fold(t.get_mut(first)?, |v, s| child_mut(v, s))
}

fn parent_mut<'a>(t: &'a mut toml::Table, path: &[Seg]) -> Option<(&'a mut Value, Seg)> {
    let (last, init) = path.split_last()?;
    if init.is_empty() {
        return None;
    }
    Some((get_mut(t, init)?, last.clone()))
}

fn remove(t: &mut toml::Table, path: &[Seg]) -> Option<Value> {
    if let [Seg::Key(k)] = path {
        return t.remove(k);
    }
    let (parent, last) = parent_mut(t, path)?;
    let removed = match (parent, last) {
        (Value::Table(p), Seg::Key(k)) => p.remove(&k),
        (Value::Array(a), Seg::Index(i)) if i < a.len() => Some(a.remove(i)),
        _ => None,
    };
    prune_empty(t, &path[..path.len() - 1]);
    removed
}

/// Drop tables left empty by a removal, so the file does not collect
/// `[section]` headers with nothing under them.
fn prune_empty(t: &mut toml::Table, path: &[Seg]) {
    let mut p = path.to_vec();
    while !p.is_empty() {
        let empty = matches!(get(t, &p), Some(Value::Table(x)) if x.is_empty());
        if !empty {
            break;
        }
        remove_shallow(t, &p);
        p.pop();
    }
}

fn remove_shallow(t: &mut toml::Table, path: &[Seg]) {
    if let [Seg::Key(k)] = path {
        t.remove(k);
    } else if let Some((Value::Table(p), Seg::Key(k))) = parent_mut(t, path) {
        p.remove(&k);
    }
}

/// Set a value, creating missing tables on the way.
fn set(t: &mut toml::Table, path: &[Seg], value: Value) -> bool {
    let Some((last, init)) = path.split_last() else {
        return false;
    };
    let mut cur = t;
    for seg in init {
        let Seg::Key(k) = seg else { return false };
        let next = cur
            .entry(k.clone())
            .or_insert_with(|| Value::Table(Default::default()));
        let Value::Table(next) = next else {
            return false;
        };
        cur = next;
    }
    match last {
        Seg::Key(k) => {
            cur.insert(k.clone(), value);
            true
        }
        Seg::Index(_) => false,
    }
}

/* ------------------------------ fixes, issues ----------------------------- */

#[derive(Debug, Clone, PartialEq)]
pub enum Fix {
    /// Delete the key; the bundled default applies again.
    Remove(Path),
    Set(Path, Value),
    Rename(Path, Path),
    /// Remove every element equal to the value from the array (by value,
    /// so several element fixes compose in any order).
    RemoveElement(Path, Value),
}

/// One problem in the user file, shown on the home screen.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Issue {
    /// Stable per problem, used to apply its fix.
    pub id: String,
    /// Where it is, as written in `config.toml`.
    pub path: String,
    pub problem: String,
    /// What the fix does; `None` when there is nothing safe to propose.
    pub fix_label: Option<String>,
    #[serde(skip)]
    pub fix: Option<Fix>,
}

impl Issue {
    fn new(path: &[Seg], problem: String, fix: Option<(Fix, String)>) -> Self {
        let p = display(path);
        let (fix, fix_label) = match fix {
            Some((f, l)) => (Some(f), Some(l)),
            None => (None, None),
        };
        Self {
            id: p.clone(),
            path: p,
            problem,
            fix_label,
            fix,
        }
    }
}

impl Issue {
    /// A value the loader dropped for a reason the schema walk did not
    /// name.
    pub fn unreadable(path: &[Seg]) -> Self {
        Self::new(
            path,
            "読込不可の値".to_string(),
            Some((Fix::Remove(path.to_vec()), "既定値に復元".to_string())),
        )
    }
}

/// `issue_path` is `path` or lies below it.
pub fn starts_with(issue_path: &str, path: &[Seg]) -> bool {
    let p = display(path);
    issue_path == p
        || issue_path
            .strip_prefix(&p)
            .is_some_and(|rest| rest.starts_with(['.', '[']))
}

/// Apply fixes to the user table; returns how many changed something.
pub fn apply(t: &mut toml::Table, fixes: &[Fix]) -> usize {
    let mut n = 0;
    for f in fixes {
        let changed = match f {
            Fix::Remove(p) => remove(t, p).is_some(),
            Fix::Set(p, v) => get(t, p) != Some(v) && set(t, p, v.clone()),
            Fix::Rename(from, to) => match remove(t, from) {
                Some(v) if get(t, to).is_none() => set(t, to, v),
                Some(_) => true,
                None => false,
            },
            Fix::RemoveElement(p, v) => match get_mut(t, p) {
                Some(Value::Array(a)) => {
                    let before = a.len();
                    a.retain(|x| x != v);
                    before != a.len()
                }
                _ => false,
            },
        };
        n += usize::from(changed);
    }
    n
}

/* ------------------------------ pass 1: rules ----------------------------- */

/// Apply the declarative rules. Returns one line per change, for the
/// user notice (Japanese) — empty when the file was already current.
pub fn migrate(t: &mut toml::Table, rules: &Rules) -> Vec<String> {
    let mut done = Vec::new();
    for r in rules.renamed {
        let (from, to) = (dotted(r.from), dotted(r.to));
        if let Some(v) = remove(t, &from) {
            if get(t, &to).is_none() {
                set(t, &to, v);
            }
            done.push(format!("項目名変更 {} → {}", r.from, r.to));
        }
    }
    for r in rules.removed {
        if remove(t, &dotted(r.path)).is_some() {
            tracing::info!("config: dropped retired key {} ({})", r.path, r.note);
            done.push(format!("廃止項目 {} を削除", r.path));
        }
    }
    if let Some(Value::Table(providers)) = t.get_mut("providers") {
        for (task, v) in providers.iter_mut() {
            let Some(Value::Array(chain)) = v.get_mut("chain") else {
                continue;
            };
            let mut changed = false;
            for item in chain.iter_mut() {
                let Some(id) = item.as_str() else { continue };
                let alias = rules
                    .provider_aliases
                    .iter()
                    .filter(|a| a.old == id)
                    .find(|a| a.tasks.contains(&task.as_str()))
                    .or_else(|| {
                        rules
                            .provider_aliases
                            .iter()
                            .find(|a| a.old == id && a.tasks.is_empty())
                    });
                if let Some(a) = alias {
                    done.push(format!("旧識別子 {} → {}（{task}）", a.old, a.new));
                    *item = Value::String(a.new.to_string());
                    changed = true;
                }
            }
            if changed {
                let mut seen = Vec::new();
                chain.retain(|x| {
                    let dup = seen.contains(x);
                    seen.push(x.clone());
                    !dup
                });
            }
        }
        if let Some(Value::Table(configs)) = providers.get_mut("configs") {
            for a in rules.provider_aliases {
                if configs.remove(a.old).is_some() {
                    done.push(format!("旧エンジン設定 {} を削除", a.old));
                }
            }
        }
    }
    done
}

/* ------------------------------ pass 2: schema ---------------------------- */

/// Check the user table against the settings JSON Schema.
pub fn check_schema(t: &toml::Table, schema: &Json) -> Vec<Issue> {
    let mut w = Walker {
        root: schema,
        issues: Vec::new(),
    };
    w.walk(&Value::Table(t.clone()), schema, &mut Vec::new());
    w.issues
}

struct Walker<'a> {
    root: &'a Json,
    issues: Vec<Issue>,
}

impl<'a> Walker<'a> {
    /// Follow `$ref`, single-entry `allOf` and `Option`'s `anyOf`.
    fn resolve(&self, mut s: &'a Json) -> &'a Json {
        for _ in 0..16 {
            if let Some(r) = s.get("$ref").and_then(Json::as_str) {
                let name = r.rsplit('/').next().unwrap_or_default();
                match self.root.get("definitions").and_then(|d| d.get(name)) {
                    Some(d) => s = d,
                    None => break,
                }
            } else if let Some([one]) = s.get("allOf").and_then(Json::as_array).map(Vec::as_slice) {
                s = one;
            } else if let Some(any) = s.get("anyOf").and_then(Json::as_array) {
                match any
                    .iter()
                    .find(|b| b.get("type").and_then(Json::as_str) != Some("null"))
                {
                    Some(b) => s = b,
                    None => break,
                }
            } else {
                break;
            }
        }
        s
    }

    fn walk(&mut self, v: &Value, schema: &'a Json, path: &mut Path) {
        let s = self.resolve(schema);
        if s.as_bool() == Some(true) {
            return;
        }
        if let Some(choices) = choices(s) {
            self.check_choice(v, &choices, path);
            return;
        }
        let types = types(s);
        if !types.is_empty() && !types.iter().any(|ty| type_matches(v, ty)) {
            self.type_issue(v, &types, path);
            return;
        }
        match v {
            Value::Integer(i) => self.check_range(*i as f64, true, s, path),
            Value::Float(f) => self.check_range(*f, false, s, path),
            Value::Table(t) => {
                let props = s.get("properties").and_then(Json::as_object);
                let extra = s.get("additionalProperties");
                for (k, child) in t {
                    path.push(key(k));
                    if let Some(p) = props.and_then(|p| p.get(k)) {
                        self.walk(child, p, path);
                    } else {
                        match extra {
                            Some(Json::Bool(false)) => self.unknown_key(t, k, props, path),
                            Some(e) if e.is_object() => self.walk(child, e, path),
                            _ => {}
                        }
                    }
                    path.pop();
                }
            }
            Value::Array(a) => {
                if let Some(items) = s.get("items").filter(|i| i.is_object()) {
                    for (i, item) in a.iter().enumerate() {
                        path.push(Seg::Index(i));
                        self.walk(item, items, path);
                        path.pop();
                    }
                }
            }
            _ => {}
        }
    }

    fn check_choice(&mut self, v: &Value, choices: &[String], path: &Path) {
        let Some(got) = v.as_str() else {
            self.type_issue(v, &["string".into()], path);
            return;
        };
        if choices.iter().any(|c| c == got) {
            return;
        }
        let problem = format!("不正な選択肢「{got}」（候補：{}）", choices.join("／"));
        let fix = match choices.iter().find(|c| c.eq_ignore_ascii_case(got)) {
            Some(c) => (
                Fix::Set(path.clone(), Value::String(c.clone())),
                format!("「{c}」に補正"),
            ),
            None => drop_fix(path, v),
        };
        self.issues.push(Issue::new(path, problem, Some(fix)));
    }

    fn type_issue(&mut self, v: &Value, expected: &[String], path: &Path) {
        let want: Vec<&str> = expected.iter().map(|t| type_label(t)).collect();
        let problem = format!(
            "型不一致（期待：{}、実際：{}）",
            want.join("／"),
            type_label(v.type_str())
        );
        // A whole float where an integer belongs: keep the number.
        let fix = match v {
            Value::Float(f) if f.fract() == 0.0 && expected.iter().any(|t| t == "integer") => (
                Fix::Set(path.clone(), Value::Integer(*f as i64)),
                format!("整数 {} に補正", *f as i64),
            ),
            _ => drop_fix(path, v),
        };
        self.issues.push(Issue::new(path, problem, Some(fix)));
    }

    fn check_range(&mut self, x: f64, integer: bool, s: &Json, path: &Path) {
        let (min, max) = bounds(s);
        let clamped = x.clamp(min.unwrap_or(f64::MIN), max.unwrap_or(f64::MAX));
        if clamped == x {
            return;
        }
        let problem = format!(
            "範囲外 {}（許容：{}〜{}）",
            num(x),
            min.map(num).unwrap_or_default(),
            max.map(num).unwrap_or_default()
        );
        let value = if integer {
            Value::Integer(clamped as i64)
        } else {
            Value::Float(clamped)
        };
        let fix = (
            Fix::Set(path.clone(), value),
            format!("{} に補正", num(clamped)),
        );
        self.issues.push(Issue::new(path, problem, Some(fix)));
    }

    fn unknown_key(
        &mut self,
        t: &toml::Table,
        k: &str,
        props: Option<&serde_json::Map<String, Json>>,
        path: &Path,
    ) {
        let near = props.and_then(|p| {
            p.keys()
                .filter(|c| !t.contains_key(*c))
                .map(|c| (edit_distance(k, c), c))
                .filter(|(d, _)| *d <= 2)
                .min()
                .map(|(_, c)| c.clone())
        });
        let fix = match near {
            Some(c) => {
                let mut to = path.clone();
                to.pop();
                to.push(key(&c));
                (Fix::Rename(path.clone(), to), format!("「{c}」に改名"))
            }
            None => (Fix::Remove(path.clone()), "削除".to_string()),
        };
        self.issues
            .push(Issue::new(path, "未知の項目".to_string(), Some(fix)));
    }
}

/// Remove a bad value: drop an array element, else fall back to the default.
fn drop_fix(path: &Path, v: &Value) -> (Fix, String) {
    match path.split_last() {
        Some((Seg::Index(_), parent)) => (
            Fix::RemoveElement(parent.to_vec(), v.clone()),
            "配列から除外".to_string(),
        ),
        _ => (Fix::Remove(path.clone()), "既定値に復元".to_string()),
    }
}

/// Allowed strings of an enum schema (`enum` or `oneOf` of `enum`/`const`).
fn choices(s: &Json) -> Option<Vec<String>> {
    let strs = |a: &Json| -> Vec<String> {
        a.as_array()
            .into_iter()
            .flatten()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect()
    };
    if let Some(e) = s.get("enum") {
        return Some(strs(e));
    }
    let one_of = s.get("oneOf")?.as_array()?;
    let mut out = Vec::new();
    for b in one_of {
        if let Some(e) = b.get("enum") {
            out.extend(strs(e));
        } else {
            out.push(b.get("const").and_then(Json::as_str)?.to_string());
        }
    }
    Some(out)
}

fn types(s: &Json) -> Vec<String> {
    match s.get("type") {
        Some(Json::String(t)) => vec![t.clone()],
        Some(Json::Array(a)) => a
            .iter()
            .filter_map(Json::as_str)
            .filter(|t| *t != "null")
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn type_matches(v: &Value, ty: &str) -> bool {
    matches!(
        (ty, v),
        ("string", Value::String(_))
            | ("integer", Value::Integer(_))
            | ("number", Value::Integer(_) | Value::Float(_))
            | ("boolean", Value::Boolean(_))
            | ("array", Value::Array(_))
            | ("object", Value::Table(_))
    )
}

fn type_label(t: &str) -> &'static str {
    match t {
        "string" => "文字列",
        "integer" => "整数",
        "number" | "float" => "数値",
        "boolean" => "真偽値",
        "array" => "配列",
        "object" | "table" => "表",
        "datetime" => "日時",
        _ => "不明",
    }
}

/// `minimum`/`maximum`, tightened by the integer `format` schemars emits.
fn bounds(s: &Json) -> (Option<f64>, Option<f64>) {
    let (fmin, fmax) = match s.get("format").and_then(Json::as_str) {
        Some("uint8") => (Some(0.0), Some(u8::MAX as f64)),
        Some("uint16") => (Some(0.0), Some(u16::MAX as f64)),
        Some("uint32") => (Some(0.0), Some(u32::MAX as f64)),
        Some("uint64" | "uint") => (Some(0.0), None),
        Some("int8") => (Some(i8::MIN as f64), Some(i8::MAX as f64)),
        Some("int16") => (Some(i16::MIN as f64), Some(i16::MAX as f64)),
        Some("int32") => (Some(i32::MIN as f64), Some(i32::MAX as f64)),
        _ => (None, None),
    };
    let min = s.get("minimum").and_then(Json::as_f64).or(fmin);
    let max = s.get("maximum").and_then(Json::as_f64).or(fmax);
    (min, max)
}

fn num(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else if f64::from(x as f32) == x {
        // An f32 setting written back as f64 (1.7000000476837158).
        format!("{}", x as f32)
    } else {
        format!("{x}")
    }
}

fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != *cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/* ----------------------------- pass 3: providers -------------------------- */

/// Task chains with their capability and Japanese label.
pub const TASKS: [(&str, Capability, &str); 8] = [
    ("translate", Capability::Translate, "翻訳"),
    ("ocr", Capability::Ocr, "OCR"),
    ("asr", Capability::Asr, "音声認識"),
    ("explain", Capability::Explain, "解説"),
    ("summarize", Capability::Summarize, "要約"),
    (
        "reverse_image_search",
        Capability::ReverseImageSearch,
        "画像検索",
    ),
    ("qr_handler", Capability::QrHandler, "QR 処理"),
    (
        "intent_classifier",
        Capability::IntentClassifier,
        "意図判定",
    ),
];

/// Check every chain against the registered providers and what each one
/// can do. `registered` maps provider id → capabilities.
pub fn check_providers(
    s: &super::Settings,
    registered: &HashMap<String, Vec<Capability>>,
) -> Vec<Issue> {
    let p = &s.providers;
    let chains = [
        &p.translate,
        &p.ocr,
        &p.asr,
        &p.explain,
        &p.summarize,
        &p.reverse_image_search,
        &p.qr_handler,
        &p.intent_classifier,
    ];
    let mut out = Vec::new();
    for ((task, cap, label), chain) in TASKS.iter().zip(chains) {
        let mut candidates: Vec<&str> = registered
            .iter()
            .filter(|(_, caps)| caps.contains(cap))
            .map(|(id, _)| id.as_str())
            .collect();
        candidates.sort_unstable();
        let hint = if candidates.is_empty() {
            String::new()
        } else {
            format!("（候補：{}）", candidates.join("、"))
        };
        let bad: Vec<(usize, &String, String)> = chain
            .chain
            .iter()
            .enumerate()
            .filter_map(|(i, id)| match registered.get(id) {
                None => Some((i, id, format!("未登録のプロバイダー「{id}」{hint}"))),
                Some(caps) if !caps.contains(cap) => {
                    Some((i, id, format!("「{id}」は{label}非対応{hint}")))
                }
                Some(_) => None,
            })
            .collect();
        let chain_path = vec![key("providers"), key(task), key("chain")];
        let all_bad = bad.len() == chain.chain.len();
        for (i, id, problem) in bad {
            let fix = if all_bad {
                (Fix::Remove(chain_path.clone()), "既定値に復元".to_string())
            } else {
                (
                    Fix::RemoveElement(chain_path.clone(), Value::String(id.clone())),
                    "チェーンから除外".to_string(),
                )
            };
            let mut path = chain_path.clone();
            path.push(Seg::Index(i));
            out.push(Issue::new(&path, problem, Some(fix)));
        }
    }
    out
}

/* ------------------------------ shared state ------------------------------ */

/// Which pass found an issue; each pass replaces only its own list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    File,
    Providers,
}

static ISSUES: Mutex<BTreeMap<Source, Vec<Issue>>> = Mutex::new(BTreeMap::new());

/// Replace one pass's issues and tell open windows
/// (`moonlens://config-issues`) when the list changed.
pub fn publish(source: Source, issues: Vec<Issue>) {
    for i in &issues {
        tracing::warn!("config issue {}: {}", i.path, i.problem);
    }
    let changed = {
        let mut all = ISSUES.lock();
        let slot = all.entry(source).or_default();
        let changed = *slot != issues;
        *slot = issues;
        changed
    };
    if changed {
        crate::notices::emit_event("moonlens://config-issues", list());
    }
}

pub fn list() -> Vec<Issue> {
    let mut seen = std::collections::HashSet::new();
    ISSUES
        .lock()
        .values()
        .flatten()
        .filter(|i| seen.insert(i.id.clone()))
        .cloned()
        .collect()
}

/// Fixes of the listed issues (all issues with a fix when `ids` is empty).
pub fn fixes_for(ids: &[String]) -> Vec<Fix> {
    list()
        .into_iter()
        .filter(|i| ids.is_empty() || ids.contains(&i.id))
        .filter_map(|i| i.fix)
        .collect()
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
