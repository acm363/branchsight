use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

pub(super) struct Entry {
    pub status: String,
    pub path: String,
    pub from: Option<String>,
}

pub(super) type Counts = (Option<u64>, Option<u64>);

const COMBINED: [&str; 2] = ["diff --cc ", "diff --combined "];

pub(super) fn entries(raw: &str) -> Vec<Entry> {
    let mut tokens = raw.split('\0');
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    while let Some(token) = tokens.next() {
        let Some(fields) = token.strip_prefix(':') else { continue };
        let letter = fields.rsplit(' ').next().and_then(|s| s.get(..1)).unwrap_or("M");
        let status = if fields.starts_with(':') { "U" } else { letter };
        let first = tokens.next().unwrap_or_default().to_owned();
        let (from, path) = if matches!(status, "R" | "C") { (Some(first), tokens.next().unwrap_or_default().to_owned()) } else { (None, first) };
        if seen.insert(path.clone()) {
            entries.push(Entry { status: status.to_owned(), path, from });
        }
    }
    entries
}

pub(super) fn split(patch: &str, entries: &[Entry]) -> Vec<String> {
    let mut owners = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        let source = e.from.as_deref().unwrap_or(&e.path);
        owners.insert(format!("diff --git {} {}", quote(&format!("a/{source}")), quote(&format!("b/{}", e.path))), i);
        owners.insert(e.path.clone(), i);
        owners.insert(quote(&e.path), i);
    }
    let mut bodies = vec![String::new(); entries.len()];
    let mut current = None;
    for line in patch.split_inclusive('\n') {
        let text = line.trim_end_matches('\n');
        if let Some(key) = section_key(text) {
            current = owners.get(key).copied();
        }
        if let Some(i) = current {
            bodies[i].push_str(line);
        }
    }
    bodies
}

fn section_key(line: &str) -> Option<&str> {
    if line.starts_with("diff --git ") {
        return Some(line);
    }
    COMBINED.iter().chain(&["* Unmerged path "]).find_map(|prefix| line.strip_prefix(prefix))
}

pub(super) fn counts(body: &str) -> Counts {
    let (mut added, mut deleted, mut in_hunk) = (0, 0, false);
    for line in body.lines() {
        if COMBINED.iter().any(|prefix| line.starts_with(prefix)) || (!in_hunk && line.starts_with("Binary files ")) {
            return (None, None);
        }
        if line.starts_with("diff --git ") {
            in_hunk = false;
        } else if line.starts_with("@@") {
            in_hunk = true;
        } else if in_hunk && line.starts_with('+') {
            added += 1;
        } else if in_hunk && line.starts_with('-') {
            deleted += 1;
        }
    }
    (Some(added), Some(deleted))
}

pub(super) fn numstat(listing: &str) -> HashMap<String, Counts> {
    let mut tokens = listing.split('\0');
    let mut counts = HashMap::new();
    while let Some(token) = tokens.next() {
        let mut fields = token.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) = (fields.next(), fields.next(), fields.next()) else { continue };
        let path = if path.is_empty() {
            tokens.next();
            tokens.next().unwrap_or_default()
        } else {
            path
        };
        counts.insert(path.to_owned(), (added.parse().ok(), deleted.parse().ok()));
    }
    counts
}

pub(super) fn file(entry: Entry, patch: String, (added, deleted): Counts) -> Value {
    json!({ "status": entry.status, "path": entry.path, "from": entry.from, "add": added, "del": deleted, "patch": patch })
}

fn quote(name: &str) -> String {
    if !name.chars().any(|c| c == '"' || c == '\\' || c < ' ' || c == '\x7f') {
        return name.to_owned();
    }
    let mut quoted = String::from('"');
    for c in name.chars() {
        match c {
            '\x07' => quoted.push_str("\\a"),
            '\x08' => quoted.push_str("\\b"),
            '\t' => quoted.push_str("\\t"),
            '\n' => quoted.push_str("\\n"),
            '\x0b' => quoted.push_str("\\v"),
            '\x0c' => quoted.push_str("\\f"),
            '\r' => quoted.push_str("\\r"),
            '"' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            c if c < ' ' || c == '\x7f' => quoted.push_str(&format!("\\{:03o}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_quoted_the_way_git_prints_them() {
        // Then.
        assert_eq!(quote("a/plain name.txt"), "a/plain name.txt");
        assert_eq!(quote("a/tab\tname"), "\"a/tab\\tname\"");
        assert_eq!(quote("a/say \"hi\""), "\"a/say \\\"hi\\\"\"");
        assert_eq!(quote("a/bell\x07"), "\"a/bell\\a\"");
        assert_eq!(quote("a/été"), "a/été");
    }

    #[test]
    fn each_section_goes_to_its_own_file_whatever_git_prints_between_them() {
        // Given.
        let raw = ":100644 000000 1 0 U\0conflict\0:000000 100644 0 2 A\0new\0:100644 120000 3 4 T\0link\0:100644 100644 5 5 R100\0old\0moved\0";
        let patch = "* Unmerged path conflict\ndiff --git a/new b/new\n+n\ndiff --git a/link b/link\n-x\ndiff --git a/link b/link\n+y\ndiff --git a/old b/moved\nrename from old\n";

        // When.
        let bodies = split(patch, &entries(raw));

        // Then.
        assert_eq!(bodies[0], "* Unmerged path conflict\n");
        assert_eq!(bodies[1], "diff --git a/new b/new\n+n\n");
        assert_eq!(bodies[2], "diff --git a/link b/link\n-x\ndiff --git a/link b/link\n+y\n");
        assert_eq!(bodies[3], "diff --git a/old b/moved\nrename from old\n");
    }

    #[test]
    fn a_conflicted_path_listed_twice_is_one_file() {
        // When.
        let listed = entries("::100644 100644 100644 1 2 0 MM\0a\0:100644 100644 1 2 M\0a\0");

        // Then.
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, "U");
    }

    #[test]
    fn counts_skip_headers_and_know_binary_files() {
        // Then.
        assert_eq!(counts("diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1 +1,2 @@\n-a\n+b\n+c\n"), (Some(2), Some(1)));
        assert_eq!(counts("diff --git a/i b/i\nBinary files a/i and b/i differ\n"), (None, None));
        assert_eq!(numstat("1\t2\tf\0-\t-\tbin\03\t0\t\0old\0new\0")["new"], (Some(3), Some(0)));
    }
}
