//! One report-scoped redactor; all untrusted report strings pass through it.
use crate::catalog::Catalog;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

pub struct Redactor {
    pub counts: BTreeMap<String, usize>,
    names: Vec<(String, String, String)>,
    environment: BTreeSet<String>,
    commands: BTreeSet<String>,
    omitted_words: BTreeSet<String>,
}
impl Redactor {
    pub fn new(names: &[(String, String)], catalog: &Catalog) -> Self {
        let mut environment = BTreeSet::new();
        for key in ["USER", "USERNAME", "HOSTNAME", "COMPUTERNAME"] {
            if let Ok(v) = std::env::var(key) {
                if !v.is_empty() {
                    environment.insert(v);
                }
            }
        }
        if let Some(home) = std::env::var_os("HOME") {
            if let Some(s) = std::path::Path::new(&home)
                .file_name()
                .and_then(|s| s.to_str())
            {
                environment.insert(s.into());
            }
        }
        if let Ok(file) = std::fs::File::open("/etc/hostname") {
            let mut hostname = String::new();
            if file.take(512).read_to_string(&mut hostname).is_ok() && !hostname.trim().is_empty() {
                environment.insert(hostname.trim().into());
            }
        }
        let mut classes = BTreeMap::<String, usize>::new();
        let mut mapped = Vec::new();
        for (name, class) in names.iter().take(256) {
            if name.is_empty() || mapped.iter().any(|(n, _, _)| n == name) {
                continue;
            }
            let n = classes.entry(class.clone()).or_default();
            *n += 1;
            mapped.push((
                name.clone(),
                format!("{class}{n}"),
                format!("{} name", class.to_lowercase()),
            ));
        }
        mapped.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
        Self {
            omitted_words: BTreeSet::new(),
            counts: BTreeMap::new(),
            names: mapped,
            environment,
            commands: catalog.commands().iter().map(|c| c.id.clone()).collect(),
        }
    }
    /// Prevent request words echoed in errors/reasons from bypassing consent.
    pub fn omit_request_words(&mut self, request: &str) {
        for word in request
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .take(4096)
        {
            if !word.is_empty() && !measurement(word) && !fixed_word(word) {
                self.omitted_words.insert(word.to_ascii_lowercase());
            }
        }
    }
    fn replacement(&mut self, class: &str, replacement: &str) -> String {
        *self.counts.entry(class.into()).or_default() += 1;
        replacement.into()
    }
    pub fn text(&mut self, text: &str) -> String {
        self.redact(text, false, false)
    }
    pub fn prose(&mut self, text: &str) -> String {
        self.redact(text, true, false)
    }
    pub fn value(&mut self, value: &serde_json::Value) -> serde_json::Value {
        fn walk(
            r: &mut Redactor,
            v: &serde_json::Value,
            key: &str,
            depth: usize,
        ) -> serde_json::Value {
            if depth > 32 {
                return serde_json::json!("[depth limit]");
            }
            match v {
                serde_json::Value::String(s) => {
                    serde_json::json!(if matches!(key, "reason" | "error" | "message") {
                        r.prose(s)
                    } else if matches!(key, "command" | "commands" | "tool" | "tools") {
                        r.redact(s, false, true)
                    } else {
                        r.text(s)
                    })
                }
                serde_json::Value::Array(a) => serde_json::Value::Array(
                    a.iter()
                        .take(256)
                        .map(|v| walk(r, v, key, depth + 1))
                        .collect(),
                ),
                serde_json::Value::Object(m) => serde_json::Value::Object(
                    m.iter()
                        .take(256)
                        .map(|(k, v)| (r.text(k), walk(r, v, k, depth + 1)))
                        .collect(),
                ),
                _ => v.clone(),
            }
        }
        walk(self, value, "", 0)
    }
    fn redact(&mut self, text: &str, contextual: bool, command_id: bool) -> String {
        let mut out = String::new();
        let mut offset = 0;
        let mut bearer = false;
        let mut hostname_next = false;
        let mut command_next = false;
        let mut name_context: Option<&str> = None;
        while offset < text.len() {
            let rest = &text[offset..];
            let found = self
                .names
                .iter()
                .find(|(name, _, _)| {
                    !command_id
                        && !command_next
                        && !self.commands.contains(
                            rest.split(delimiter)
                                .next()
                                .unwrap_or("")
                                .trim_end_matches(['.', ':', '!', '?']),
                        )
                        && rest.starts_with(name)
                        && boundary(text, offset)
                        && boundary(text, offset + name.len())
                })
                .cloned();
            if let Some((name, replacement, class)) = found {
                out.push_str(&self.replacement(&class, &replacement));
                offset += name.len();
                name_context = None;
                continue;
            }
            if let Some(quote) = rest
                .chars()
                .next()
                .filter(|c| matches!(c, '"' | '\'' | '`'))
            {
                if let Some(end) = rest[quote.len_utf8()..]
                    .find(quote)
                    .map(|n| n + quote.len_utf8())
                {
                    let inner = &rest[quote.len_utf8()..end];
                    if inner.starts_with('/')
                        || inner.starts_with("./")
                        || inner.starts_with("../")
                        || inner.starts_with("~/")
                        || inner.as_bytes().get(1) == Some(&b':') && inner.contains('\\')
                    {
                        out.push(quote);
                        out.push_str(&self.replacement("path", "[path]"));
                        out.push(quote);
                        offset += end + quote.len_utf8();
                        name_context = None;
                        continue;
                    }
                }
            }
            if let Some(class) = name_context {
                if let Some(quote) = rest
                    .chars()
                    .next()
                    .filter(|c| matches!(c, '"' | '\'' | '`'))
                {
                    if let Some(end) = rest[quote.len_utf8()..]
                        .find(quote)
                        .map(|n| n + quote.len_utf8())
                    {
                        let name = &rest[quote.len_utf8()..end];
                        if !dimension_phrase(name) && !name.is_empty() {
                            let replacement = self.learn_name(name, class);
                            out.push(quote);
                            out.push_str(&self.replacement(
                                &format!("{} name", class.to_lowercase()),
                                &replacement,
                            ));
                            out.push(quote);
                            offset += end + quote.len_utf8();
                            name_context = None;
                            continue;
                        }
                    }
                }
            }
            let c = rest.chars().next().unwrap_or(' ');
            if delimiter(c) {
                out.push(c);
                offset += c.len_utf8();
                continue;
            }
            let end = rest.find(delimiter).unwrap_or(rest.len());
            let raw = &rest[..end];
            let token = raw.trim_end_matches(['.', ':', '!', '?']);
            let suffix = &raw[token.len()..];
            let lower = token.to_ascii_lowercase();
            let class = if bearer {
                Some("token")
            } else if lower.starts_with("http://")
                || lower.starts_with("https://")
                || lower.starts_with("ftp://")
                || lower.starts_with("www.")
            {
                Some("URL")
            } else if token.contains('@') {
                Some("email")
            } else if token.parse::<std::net::IpAddr>().is_ok()
                || token
                    .rsplit_once(':')
                    .is_some_and(|(ip, _)| ip.parse::<std::net::IpAddr>().is_ok())
            {
                Some("IP address")
            } else if self.commands.contains(token) || measurement(token) {
                None
            } else if path(token) {
                Some("path")
            } else if secret(token) || token.split(':').any(secret) {
                Some("token")
            } else if self.environment.contains(token) {
                Some("environment name")
            } else if !command_id
                && !command_next
                && ((hostname_next && !fixed_word(token))
                    || lower.starts_with("host:")
                    || lower.starts_with("hostname:")
                    || (!self.commands.contains(token)
                        && (lower == "localhost" || hostname(token))))
            {
                Some("hostname")
            } else if !command_id && !command_next && self.omitted_words.contains(&lower) {
                Some("request word")
            } else {
                None
            };
            bearer = lower.rsplit(':').next() == Some("bearer");
            hostname_next = matches!(lower.as_str(), "host" | "hostname" | "server");
            command_next = contextual && matches!(lower.as_str(), "command" | "commands");
            let previous_context = name_context.take();
            if let Some(class) = class {
                let replacement = if class == "request word" {
                    "[word]".into()
                } else {
                    format!("[{class}]")
                };
                out.push_str(&self.replacement(class, &replacement));
            } else if let Some(class) = previous_context
                .filter(|_| !measurement(token) && !token.is_empty() && !fixed_word(token))
            {
                let replacement = self.learn_name(token, class);
                out.push_str(
                    &self.replacement(&format!("{} name", class.to_lowercase()), &replacement),
                );
            } else {
                out.push_str(token);
            }
            name_context = if !contextual {
                None
            } else {
                match lower.as_str() {
                    "body" => Some("Body"),
                    "component" => Some("Component"),
                    "sketch" => Some("Sketch"),
                    "parameter" => Some("Param"),
                    "file" => Some("File"),
                    "named" | "called" | "name" | "to" if previous_context.is_some() => {
                        previous_context
                    }
                    _ => None,
                }
            };
            out.push_str(suffix);
            offset += end;
        }
        out
    }
    fn learn_name(&mut self, name: &str, class: &str) -> String {
        if let Some((_, replacement, _)) = self.names.iter().find(|(n, _, _)| n == name) {
            return replacement.clone();
        }
        let category = format!("{} name", class.to_lowercase());
        let n = self.names.iter().filter(|(_, _, c)| c == &category).count() + 1;
        let replacement = format!("{class}{n}");
        if self.names.len() < 512 {
            self.names
                .push((name.into(), replacement.clone(), category));
            self.names.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
        }
        replacement
    }
    pub fn summary(&self) -> String {
        if self.counts.is_empty() {
            return "redacted: none".into();
        }
        format!(
            "redacted: {}",
            self.counts
                .iter()
                .map(|(kind, n)| format!(
                    "{n} {kind}{}",
                    if *n == 1 {
                        ""
                    } else if kind.ends_with("address") {
                        "es"
                    } else {
                        "s"
                    }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
fn delimiter(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '=' | '"'
                | '\''
                | '`'
                | '('
                | ')'
                | '{'
                | '}'
                | '['
                | ']'
                | ','
                | '|'
                | ';'
                | '<'
                | '>'
        )
}
fn boundary(s: &str, i: usize) -> bool {
    i == 0
        || i == s.len()
        || s[..i]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_alphanumeric() && c != '_' && c != '-')
        || s[i..]
            .chars()
            .next()
            .is_some_and(|c| !c.is_alphanumeric() && c != '_' && c != '-')
}
fn numeric(s: &str) -> bool {
    fn scalar(s: &str) -> bool {
        s.parse::<f64>().is_ok()
            || s.split_once('/')
                .is_some_and(|(n, d)| n.parse::<f64>().is_ok() && d.parse::<f64>().is_ok())
    }
    scalar(s) || (s.contains(['x', '×']) && s.split(['x', '×']).all(scalar))
}
fn measurement(s: &str) -> bool {
    let s = s.to_ascii_lowercase();
    let number = ["inches", "inch", "mm3", "mm", "cm", "m", "in"]
        .iter()
        .find_map(|unit| s.strip_suffix(unit))
        .unwrap_or(&s);
    !number.is_empty() && numeric(number)
}
// Contextual labels must not eat Pilot's own phrases or ordinary dimensions.
fn dimension_phrase(s: &str) -> bool {
    s.split_whitespace().all(|w| {
        measurement(w)
            || matches!(
                w,
                "mm" | "cm" | "m" | "inch" | "inches" | "in" | "x" | "×" | "by"
            )
    })
}
fn fixed_word(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if crate::planner::report_tokens(&lower)
        .iter()
        .any(|word| word == &lower)
    {
        return true;
    }
    matches!(
        s.to_ascii_lowercase().as_str(),
        "missing"
            | "ambiguous"
            | "slots"
            | "state"
            | "tool"
            | "offer"
            | "supported"
            | "unambiguous"
            | "plan"
            | "recipe"
            | "matched"
            | "declined"
            | "multiple"
            | "intents"
            | "request"
            | "exceeds"
            | "planner"
            | "text"
            | "limit"
            | "guarded"
            | "unsupported"
            | "modifier"
            | "rectangle"
            | "circle"
            | "plane"
            | "xy"
            | "xz"
            | "yz"
            | "name"
            | "named"
            | "called"
            | "names"
            | "count"
            | "volume"
            | "size"
            | "types"
            | "type"
            | "result"
            | "results"
            | "error"
            | "errors"
            | "not"
            | "is"
            | "was"
            | "has"
            | "with"
            | "the"
            | "a"
            | "an"
            | "all"
            | "to"
            | "from"
            | "for"
            | "of"
            | "and"
            | "does"
            | "could"
            | "cannot"
            | "unchanged"
            | "open"
            | "closed"
            | "failed"
            | "unknown"
    )
}
fn path(s: &str) -> bool {
    (s.contains('/') || s.contains('\\')) && !measurement(s)
        || s.rsplit_once('.').is_some_and(|(_, ext)| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "json"
                    | "step"
                    | "stp"
                    | "stl"
                    | "obj"
                    | "solidcraft"
                    | "txt"
                    | "rs"
                    | "md"
                    | "cad"
                    | "iges"
                    | "pdf"
                    | "csv"
                    | "log"
            )
        })
}
fn hostname(s: &str) -> bool {
    let s = s
        .rsplit_once(':')
        .filter(|(_, port)| port.parse::<u16>().is_ok())
        .map_or(s, |(host, _)| host);
    if numeric(s) {
        return false;
    }
    s.split('.').count() >= 2
        && s.split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        && s.rsplit('.')
            .next()
            .is_some_and(|p| p.len() >= 2 && p.chars().all(|c| c.is_ascii_alphabetic()))
}
fn secret(s: &str) -> bool {
    if [
        "sk-",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "github_pat_",
        "xox",
        "AKIA",
    ]
    .iter()
    .any(|p| s.starts_with(p))
    {
        return true;
    }
    if s.len() < 24
        || !s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-=+/.".contains(&c))
    {
        return false;
    }
    let mut counts = BTreeMap::<u8, usize>::new();
    for b in s.bytes() {
        *counts.entry(b).or_default() += 1;
    }
    let n = s.len() as f64;
    let entropy: f64 = counts
        .values()
        .map(|c| {
            let p = *c as f64 / n;
            -p * p.log2()
        })
        .sum();
    entropy >= 3.5 && s.bytes().any(|c| c.is_ascii_alphabetic())
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    #[test]
    fn classes_and_safe_dimensions() {
        let mut r = Redactor::new(
            &[
                ("Secret Widget".into(), "Body".into()),
                ("widthSecret".into(), "Param".into()),
            ],
            &Catalog::bundled().unwrap(),
        );
        r.environment.insert("privateuser".into());
        let s=r.text("/home/private/a.step ./part.json ../x C:\\Users\\a.step relative/part.stl https://example.com/x a@b.com 192.168.1.2 ::1 server.internal privateuser sk-abc ghp_abc gho_abc xoxb-abc AKIA123 Bearer xyz A1b2C3d4E5f6G7h8I9j0K1l2 Secret Widget Secret Widget widthSecret");
        for private in [
            "/home/private",
            "../x",
            "./part.json",
            "relative/part.stl",
            "C:\\Users",
            "a@b.com",
            "192.168",
            "server.internal",
            "privateuser",
            "sk-abc",
            "xyz",
            "A1b2",
            "Secret Widget",
            "widthSecret",
        ] {
            assert!(!s.contains(private), "{s}");
        }
        assert!(s.contains("Body1 Body1 Param1"));
        let safe="20 3.5 20mm 1/4 1/4inch \"20 mm\" solid.cube sketch.begin 123456789012345678901234567890";
        // Commands are protected using the public catalogue, including real ids.
        r.commands.insert("solid.cube".into());
        r.commands.insert("sketch.begin".into());
        assert_eq!(r.text(safe), safe);
        assert!(r.counts.get("path").copied().unwrap_or(0) >= 5);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod more_tests {
    use super::*;
    #[test]
    fn names_paths_and_fixed_text() {
        let mut r = Redactor::new(&[], &Catalog::bundled().unwrap());
        assert_eq!(
            r.prose("body 'Private Gear' and body 'Private Gear'"),
            "body 'Body1' and body 'Body1'"
        );
        assert_eq!(
            r.prose("body count unchanged; sketch rectangle 20 mm; parameter '20 mm'"),
            "body count unchanged; sketch rectangle 20 mm; parameter '20 mm'"
        );
        assert_eq!(
            r.text("\"/home/person/My Private Design/model.step\""),
            "\"[path]\""
        );
        let value = serde_json::json!({"body":"Private Gear","slots":{"Sketch":{"plane":"XY","size":[20,10],"distance":5}}});
        let commands = r.value(&serde_json::json!({"commands":["solid.unknown_test"]}));
        assert_eq!(commands["commands"][0], "solid.unknown_test");
        assert_eq!(
            r.prose("unknown command solid.unknown_test"),
            "unknown command solid.unknown_test"
        );
        let value = r.value(&value);
        assert_eq!(value["body"], "Body1");
        assert_eq!(value["slots"]["Sketch"]["plane"], "XY");
        assert_eq!(
            r.text("token=sk-test-secret host.local:7878 user@example.org"),
            "token=[token] [hostname] [email]"
        );
    }
    #[test]
    fn entropy_and_prefixes() {
        let mut r = Redactor::new(&[], &Catalog::bundled().unwrap());
        for token in [
            "sk-test",
            "ghp_test",
            "gho_test",
            "xoxb-test",
            "AKIA1234567890",
            "aBcdEfGhIjKlMnOpQrStUvWxYzABCDEF",
            "9e1ca07b8d4f2350aabf8dcb136e95f2",
        ] {
            assert_eq!(r.text(token), "[token]", "{token}");
        }
        for command in Catalog::bundled().unwrap().commands() {
            assert_eq!(r.text(&command.id), command.id);
        }
        for dimension in [
            "20mm",
            "1/4inch",
            "1/8",
            "123456789012345678901234567890",
            "\"20 mm\"",
            "-3.125",
            "84x36x9mm",
        ] {
            assert_eq!(r.text(dimension), dimension);
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod name_classes {
    use super::*;
    #[test]
    fn paths_take_precedence_over_entropy_tokens() {
        for path in [
            "/home/USER/Projects/secret/bracket.solidcraft",
            r"C:\Users\USER\Projects\secret\bracket.solidcraft",
            "~/Projects/secret/bracket.solidcraft",
        ] {
            // The POSIX path meets the entropy rule, so ordering matters.
            if path.starts_with('/') {
                assert!(secret(path));
            }
            for text in [
                path.to_string(),
                format!("\"{path}\""),
                format!("save to {path}."),
            ] {
                let mut r = Redactor::new(&[], &Catalog::bundled().unwrap());
                let redacted = r.prose(&text);
                assert!(redacted.contains("[path]"), "{redacted}");
                assert!(!redacted.contains("[token]"));
                assert_eq!(r.counts.get("path"), Some(&1));
                assert!(!r.counts.contains_key("token"));
            }
        }
    }
    #[test]
    fn each_name_class_is_consistent() {
        let names = [
            ("Rotor", "Body"),
            ("Assembly", "Component"),
            ("Outline", "Sketch"),
            ("Pitch", "Param"),
            ("Drawing", "File"),
        ]
        .map(|(n, c)| (n.to_string(), c.to_string()));
        let mut r = Redactor::new(&names, &Catalog::bundled().unwrap());
        assert_eq!(
            r.text("Rotor Assembly Outline Pitch Drawing Rotor"),
            "Body1 Component1 Sketch1 Param1 File1 Body1"
        );
        assert_eq!(r.counts.get("body name"), Some(&2));
    }
}
