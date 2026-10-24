//! Conservative parsing: only displayed targets in the active context are usable.
use serde::Serialize;
#[derive(Clone, Debug, Default, Serialize)]
pub struct Screen {
    pub observation: u64,
    pub bounds: Option<(u32, u32)>,
    pub dialog: Option<String>,
    pub active_window: Option<String>,
    pub focus: Option<String>,
    pub selection: Option<String>,
    pub menu: Option<String>,
    pub targets: Vec<String>,
    pub timeline: Vec<String>,
    pub tutor: Option<String>,
    pub offer: Option<String>,
    pub incomplete: bool,
}
pub fn normalize(s: &str) -> String {
    s.replace('&', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches([':', '.', '…'])
        .trim()
        .to_lowercase()
}
fn quoted(s: &str) -> Option<String> {
    s.split_once('"')
        .and_then(|(_, r)| r.split_once('"'))
        .map(|(s, _)| s.to_owned())
}
impl Screen {
    pub fn nearest(&self, name: &str) -> Vec<String> {
        if name.len() > 256 {
            return Vec::new();
        }
        let want = normalize(name);
        let mut ranked = Vec::new();
        for target in &self.targets {
            let candidate = normalize(target);
            let mut row: Vec<usize> = (0..=candidate.chars().count()).collect();
            for (i, a) in want.chars().enumerate() {
                let mut diagonal = row[0];
                row[0] = i + 1;
                for (j, b) in candidate.chars().enumerate() {
                    let old = row[j + 1];
                    row[j + 1] = (row[j] + 1)
                        .min(old + 1)
                        .min(diagonal + usize::from(a != b));
                    diagonal = old;
                }
            }
            let distance = row.last().copied().unwrap_or(257);
            let overlap = want
                .split_whitespace()
                .filter(|w| candidate.split_whitespace().any(|c| c == *w))
                .count();
            if distance <= 4 || overlap > 0 {
                ranked.push((distance, std::cmp::Reverse(overlap), target.clone()));
            }
        }
        ranked.sort();
        ranked.dedup();
        ranked.into_iter().take(3).map(|(_, _, s)| s).collect()
    }
    pub fn parse(text: &str, observation: u64) -> Result<Self, String> {
        if text.len() > super::protocol::ANSWER {
            return Err("screen exceeds answer cap".into());
        }
        let mut s = Self {
            observation,
            ..Self::default()
        };
        if let Some(size) = text
            .lines()
            .next()
            .and_then(|l| l.strip_prefix("SolidCraft 98, "))
        {
            if let Some((w, h)) = size.split_once(" x ") {
                let h = h.split('.').next().unwrap_or("");
                if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                    if w == 0 || h == 0 || w > 16384 || h > 16384 {
                        return Err("screen bounds outside 1–16384".into());
                    }
                    s.bounds = Some((w, h));
                }
            }
        }
        let mut in_menu = false;
        let mut tree = false;
        let mut dialog_controls = true;
        let mut list = false;
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with("Dialog ") || (t.starts_with("Window ") && t.contains("on top")) {
                if s.dialog.is_some() {
                    dialog_controls = false;
                    continue;
                }
                s.targets.clear();
                s.dialog = quoted(t);
            }
            if t.starts_with("Underneath:") {
                dialog_controls = false;
                continue;
            }
            if t.starts_with("Part window ") {
                tree = t.contains("(the active one)");
                if tree {
                    s.active_window = quoted(t);
                }
            }
            if t.contains("has the focus") {
                s.focus = Some(t.into());
            }
            if t.contains("<- selected") {
                s.selection = Some(t.into());
            }
            if let Some(v) = t.strip_prefix("Tutor: ") {
                s.tutor = Some(v.into());
            }
            if let Some((_, v)) = t.split_once("Say \"yes\" to do: ") {
                s.offer = Some(v.into());
            }
            if t.contains("more below") || t.contains("scrolled out of view") {
                s.incomplete = true;
            }
            let mut names = Vec::new();
            if s.dialog.is_some() {
                if !dialog_controls {
                    continue;
                }
                if t.starts_with("list") {
                    list = true;
                } else if list && line.starts_with("    ") && !t.starts_with('(') {
                    names.push(
                        t.trim_start_matches("[folder] ")
                            .split("  (selected)")
                            .next()
                            .unwrap_or(t)
                            .into(),
                    );
                } else if line.starts_with("  ") {
                    list = false;
                }
                if t.starts_with("field \"") {
                    names.extend(quoted(t));
                }
                if let Some(rest) = t.strip_prefix("button [") {
                    if let Some((name, _)) = rest.split_once(']') {
                        names.push(name.into());
                    }
                }
                if t.starts_with("title bar close box") {
                    names.push("x".into());
                }
            } else {
                if tree && t.starts_with("View (x ") {
                    names.push("view".into());
                }
                if let Some(v) = t
                    .strip_prefix("Menu bar: ")
                    .or_else(|| t.strip_prefix("Toolbar buttons: "))
                {
                    names.extend(
                        v.split(", ")
                            .map(|n| n.trim_end_matches(" (pressed in)").to_owned()),
                    );
                    in_menu = false;
                } else if t.starts_with("The ") && t.ends_with(" menu is open:") {
                    s.menu = Some(t.into());
                    in_menu = true;
                } else if in_menu && line.starts_with("  ") {
                    names.push(
                        t.trim_start_matches("[x] ")
                            .split("   (")
                            .next()
                            .unwrap_or(t)
                            .into(),
                    );
                }
                if tree && t.contains(" | ") {
                    s.timeline.push(t.into());
                    if !t.contains("scrolled out of view") {
                        if let Some(name) = t.split(" | ").nth(1) {
                            names.push(name.into());
                        }
                    }
                }
            }
            for name in names {
                if name.len() > 256 || s.targets.len() >= 512 {
                    return Err("screen target count or name exceeds cap".into());
                }
                s.targets.push(name);
            }
        }
        Ok(s)
    }
    pub fn resolve(&self, name: &str) -> Result<String, String> {
        if name.len() > 256 {
            return Err("target name exceeds 256 bytes".into());
        }
        let norm = normalize(name);
        let found: Vec<_> = self
            .targets
            .iter()
            .filter(|s| normalize(s) == norm)
            .collect();
        match found.as_slice() {
            [one] => Ok((*one).clone()),
            [] => Err(format!(
                "No displayed target named {name:?}; inspect look and choose a displayed name"
            )),
            _ => Err(format!(
                "Ambiguous target {name:?}; use coordinates to choose"
            )),
        }
    }
}
