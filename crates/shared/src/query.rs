//! The issue search syntax, e.g. `is:issue state:open label:"good first issue" author:@me crash`.
//! Parsed here so the server and the CLI agree on it.

use crate::IssueState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Created,
    Updated,
    Comments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Missing {
    Label,
    Assignee,
    Milestone,
    Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueQuery {
    /// `None` means open and closed.
    pub state: Option<IssueState>,
    /// Every one of these labels must be present.
    pub labels: Vec<String>,
    /// None of these labels may be present (`-label:x`).
    pub not_labels: Vec<String>,
    /// `no:label`, `no:assignee`, `no:milestone`, `no:type`.
    pub missing: Vec<Missing>,
    pub author: Option<String>,
    pub assignee: Option<String>,
    pub milestone: Option<String>,
    pub issue_type: Option<String>,
    pub sort: SortField,
    pub ascending: bool,
    /// `in:title` / `in:body`; both when neither is given.
    pub in_title: bool,
    pub in_body: bool,
    /// Words that are not qualifiers.
    pub text: String,
}

impl Default for IssueQuery {
    fn default() -> Self {
        Self {
            state: None,
            labels: Vec::new(),
            not_labels: Vec::new(),
            missing: Vec::new(),
            author: None,
            assignee: None,
            milestone: None,
            issue_type: None,
            sort: SortField::Created,
            ascending: false,
            in_title: true,
            in_body: true,
            text: String::new(),
        }
    }
}

/// Splits on whitespace, keeping `"quoted phrases"` (also after `key:`) together.
pub fn tokenize(q: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in q.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                current.push(c);
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn unquote(value: &str) -> String {
    value.trim_matches('"').to_string()
}

pub fn parse(q: &str) -> IssueQuery {
    let mut out = IssueQuery::default();
    let mut words: Vec<String> = Vec::new();
    let mut scoped = (false, false);
    for token in tokenize(q) {
        let (negated, body) = match token.strip_prefix('-') {
            Some(rest) if rest.contains(':') => (true, rest),
            _ => (false, token.as_str()),
        };
        let Some((key, raw)) = body.split_once(':') else {
            words.push(token);
            continue;
        };
        let value = unquote(raw);
        let lower = value.to_lowercase();
        let known = match (key, negated) {
            ("is" | "state", false) => match lower.as_str() {
                "open" => {
                    out.state = Some(IssueState::Open);
                    true
                }
                "closed" => {
                    out.state = Some(IssueState::Closed);
                    true
                }
                "issue" => key == "is",
                _ => false,
            },
            ("label", false) if !value.is_empty() => {
                out.labels.push(value.clone());
                true
            }
            ("label", true) if !value.is_empty() => {
                out.not_labels.push(value.clone());
                true
            }
            ("no", false) => {
                let field = match lower.as_str() {
                    "label" => Some(Missing::Label),
                    "assignee" => Some(Missing::Assignee),
                    "milestone" => Some(Missing::Milestone),
                    "type" => Some(Missing::Type),
                    _ => None,
                };
                if let Some(f) = field {
                    out.missing.push(f);
                }
                field.is_some()
            }
            ("author", false) if !value.is_empty() => {
                out.author = Some(value.clone());
                true
            }
            ("assignee", false) if !value.is_empty() => {
                out.assignee = Some(value.clone());
                true
            }
            ("milestone", false) if !value.is_empty() => {
                out.milestone = Some(value.clone());
                true
            }
            ("type", false) if !value.is_empty() => {
                out.issue_type = Some(value.clone());
                true
            }
            ("in", false) => match lower.as_str() {
                "title" => {
                    scoped.0 = true;
                    true
                }
                "body" => {
                    scoped.1 = true;
                    true
                }
                _ => false,
            },
            ("sort", false) => {
                let (field, dir) = lower.split_once('-').unwrap_or((lower.as_str(), "desc"));
                let field = match field {
                    "created" => Some(SortField::Created),
                    "updated" => Some(SortField::Updated),
                    "comments" => Some(SortField::Comments),
                    _ => None,
                };
                match (field, dir) {
                    (Some(f), "asc" | "desc") => {
                        out.sort = f;
                        out.ascending = dir == "asc";
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        };
        if !known {
            words.push(token);
        }
    }
    if scoped.0 || scoped.1 {
        out.in_title = scoped.0;
        out.in_body = scoped.1;
    }
    out.text = words.iter().map(|w| unquote(w)).collect::<Vec<_>>().join(" ");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_quoted_values() {
        assert_eq!(
            tokenize(r#"is:open  label:"good first issue" "exact phrase" word"#),
            ["is:open", r#"label:"good first issue""#, r#""exact phrase""#, "word"]
        );
    }

    #[test]
    fn parses_qualifiers_and_text() {
        let q = parse(r#"is:issue state:closed label:bug label:"help wanted" -label:wontfix no:assignee author:@me milestone:"v1.0" type:Bug sort:updated-asc in:title crash on start"#);
        assert_eq!(q.state, Some(IssueState::Closed));
        assert_eq!(q.labels, ["bug", "help wanted"]);
        assert_eq!(q.not_labels, ["wontfix"]);
        assert_eq!(q.missing, [Missing::Assignee]);
        assert_eq!(q.author.as_deref(), Some("@me"));
        assert_eq!(q.milestone.as_deref(), Some("v1.0"));
        assert_eq!(q.issue_type.as_deref(), Some("Bug"));
        assert_eq!((q.sort, q.ascending), (SortField::Updated, true));
        assert_eq!((q.in_title, q.in_body), (true, false));
        assert_eq!(q.text, "crash on start");
    }

    #[test]
    fn defaults_and_unknown_qualifiers() {
        let q = parse("");
        assert_eq!(q, IssueQuery::default());
        // Things that only look like qualifiers stay searchable text.
        let q = parse("http://x.test foo:bar is:weird sort:nope -dash");
        assert_eq!(q.text, "http://x.test foo:bar is:weird sort:nope -dash");
        assert_eq!(q.state, None);
        assert_eq!(parse("is:open").state, Some(IssueState::Open));
        assert_eq!(parse("sort:comments").sort, SortField::Comments);
    }
}
