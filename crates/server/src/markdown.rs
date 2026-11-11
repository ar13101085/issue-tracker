//! Markdown to safe HTML, GitHub flavoured.
//!
//! comrak renders (tables, task lists, strikethrough, autolinks, emoji
//! shortcodes, raw HTML), ammonia strips anything unsafe, then `@user` and
//! `#12` in plain text become links when the user or issue exists.

use crate::error::ApiError;
use crate::models::{IssueDoc, RepoDoc, UserDoc};
use crate::state::AppState;
use futures::TryStreamExt;
use mongodb::bson::{doc, oid::ObjectId};
use regex::{Captures, Regex};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::LazyLock;

pub struct Rendered {
    pub html: String,
    /// Existing users mentioned with `@login`.
    pub mentions: Vec<ObjectId>,
    /// Existing issues of the same repository referenced with `#number`.
    pub issue_refs: Vec<i64>,
}

static MENTION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^\w@/&.`])@([A-Za-z0-9](?:-?[A-Za-z0-9]){0,38})").unwrap());
static ISSUE_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|[^\w&/#])#([1-9]\d{0,9})\b").unwrap());

static CLEANER: LazyLock<ammonia::Builder<'static>> = LazyLock::new(|| {
    let mut b = ammonia::Builder::default();
    b.add_tags(["input", "details", "summary", "del", "ins", "kbd", "sub", "sup", "hr"])
        .add_tag_attributes("input", ["type", "checked", "disabled"])
        .add_tag_attributes("code", ["class"])
        .add_tag_attributes("li", ["class"])
        .add_tag_attributes("ul", ["class"])
        .add_tag_attributes("details", ["open"])
        .add_generic_attributes(["align"])
        .set_tag_attribute_value("input", "type", "checkbox")
        .set_tag_attribute_value("input", "disabled", "")
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .link_rel(Some("nofollow noopener noreferrer"))
        // Only the classes comrak itself writes survive.
        .attribute_filter(|_, attribute, value| {
            if attribute != "class" {
                return Some(value.into());
            }
            let kept: Vec<&str> = value
                .split_whitespace()
                .filter(|c| c.starts_with("language-") || matches!(*c, "task-list-item" | "contains-task-list"))
                .collect();
            if kept.is_empty() {
                None
            } else {
                Some(kept.join(" ").into())
            }
        });
    b
});

/// Markdown to sanitized HTML, without touching the database.
pub fn to_html(text: &str) -> String {
    let mut options = comrak::Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.shortcodes = true;
    options.render.hardbreaks = true;
    options.render.r#unsafe = true;
    options.render.tasklist_classes = true;
    CLEANER.clean(&comrak::markdown_to_html(text, &options)).to_string()
}

/// Calls `f` on every run of text that is not inside a link, code or pre element.
fn map_text(html: &str, mut f: impl FnMut(&str) -> String) -> String {
    let mut out = String::with_capacity(html.len());
    let mut skip_depth = 0usize;
    let mut rest = html;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            out.push_str(&if skip_depth == 0 { f(rest) } else { rest.to_string() });
            break;
        };
        let (text, tail) = rest.split_at(open);
        if !text.is_empty() {
            out.push_str(&if skip_depth == 0 { f(text) } else { text.to_string() });
        }
        let end = tail.find('>').map_or(tail.len(), |i| i + 1);
        let tag = &tail[..end];
        let closing = tag.starts_with("</");
        let name: String =
            tag.trim_start_matches("</").trim_start_matches('<').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        if matches!(name.as_str(), "a" | "code" | "pre") {
            if closing {
                skip_depth = skip_depth.saturating_sub(1);
            } else if !tag.ends_with("/>") {
                skip_depth += 1;
            }
        }
        out.push_str(tag);
        rest = &tail[end..];
    }
    out
}

pub async fn render(state: &AppState, text: &str, repo: Option<&RepoDoc>) -> Result<Rendered, ApiError> {
    let html = to_html(text);

    // First pass: what could be a mention or a reference.
    let mut logins = BTreeSet::new();
    let mut numbers = BTreeSet::new();
    map_text(&html, |t| {
        for c in MENTION.captures_iter(t) {
            logins.insert(c[2].to_lowercase());
        }
        for c in ISSUE_REF.captures_iter(t) {
            if let Ok(n) = c[2].parse::<i64>() {
                numbers.insert(n);
            }
        }
        String::new()
    });

    let mut users: HashMap<String, UserDoc> = HashMap::new();
    if !logins.is_empty() {
        let list: Vec<String> = logins.into_iter().collect();
        let found: Vec<UserDoc> = state.db.users.find(doc! {"login_lower": {"$in": list}}).await?.try_collect().await?;
        users = found.into_iter().map(|u| (u.login_lower.clone(), u)).collect();
    }
    let mut existing: HashSet<i64> = HashSet::new();
    if let (Some(repo), false) = (repo, numbers.is_empty()) {
        let list: Vec<i64> = numbers.into_iter().collect();
        let found: Vec<IssueDoc> =
            state.db.issues.find(doc! {"repo_id": repo.id, "number": {"$in": list}}).await?.try_collect().await?;
        existing = found.into_iter().map(|i| i.number).collect();
    }

    // Second pass: link the ones that exist.
    let html = map_text(&html, |t| {
        let t = MENTION.replace_all(t, |c: &Captures| match users.get(&c[2].to_lowercase()) {
            Some(u) => format!(r#"{}<a class="user-mention" href="/{}">@{}</a>"#, &c[1], u.login, u.login),
            None => c[0].to_string(),
        });
        let t = ISSUE_REF.replace_all(&t, |c: &Captures| match (repo, c[2].parse::<i64>()) {
            (Some(r), Ok(n)) if existing.contains(&n) => {
                format!(r#"{}<a class="issue-link" href="/{}/{}/issues/{n}">#{n}</a>"#, &c[1], r.owner_login, r.name)
            }
            _ => c[0].to_string(),
        });
        t.into_owned()
    });

    let mut issue_refs: Vec<i64> = existing.into_iter().collect();
    issue_refs.sort_unstable();
    Ok(Rendered { html, mentions: users.into_values().map(|u| u.id).collect(), issue_refs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_github_flavoured_markdown() {
        let html = to_html("# Title\n\n- [x] done\n- [ ] todo\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n~~old~~ :tada:\nnext line");
        assert!(html.contains("<h1>Title</h1>"), "{html}");
        assert!(html.contains(r#"type="checkbox""#) && html.contains("checked"), "{html}");
        assert!(html.contains("<table>") && html.contains("<del>old</del>"), "{html}");
        assert!(html.contains('🎉') && html.contains("<br"), "{html}");
        let code = to_html("```rust\nfn main() {}\n```");
        assert!(code.contains(r#"class="language-rust""#), "{code}");
    }

    #[test]
    fn strips_dangerous_html() {
        let html = to_html(
            "<script>alert(1)</script><img src=x onerror=alert(1)><a href=\"javascript:alert(1)\">x</a>\
             <iframe src=\"https://e.test\"></iframe><input type=\"text\" name=\"password\"><div class=\"evil\" style=\"position:fixed\">hi</div>\
             <details open><summary>s</summary>body</details>\n\n[link](javascript:alert(2))",
        );
        for bad in ["<script", "onerror", "href=\"javascript", "<iframe", "type=\"text\"", "evil", "style="] {
            assert!(!html.contains(bad), "{bad} survived: {html}");
        }
        assert!(html.contains("<details open") && html.contains("<summary>s</summary>"), "{html}");
        assert!(html.contains(r#"type="checkbox""#), "inputs are forced to disabled checkboxes: {html}");
    }

    #[test]
    fn map_text_skips_links_and_code() {
        let seen = std::cell::RefCell::new(Vec::new());
        let out = map_text("<p>one <a href=\"/x\">two</a> <code>three</code> four</p><pre><code>five</code></pre>", |t| {
            seen.borrow_mut().push(t.to_string());
            t.to_uppercase()
        });
        assert_eq!(*seen.borrow(), ["one ", " ", " four"]);
        assert!(out.contains("ONE") && out.contains(">two<") && out.contains(">three<") && out.contains("five"));
    }

    #[test]
    fn mention_and_reference_patterns() {
        let logins = |t: &str| MENTION.captures_iter(t).map(|c| c[2].to_string()).collect::<Vec<_>>();
        assert_eq!(logins("@alice and (@bob-2), not a@b.c or x/@y"), ["alice", "bob-2"]);
        let refs = |t: &str| ISSUE_REF.captures_iter(t).map(|c| c[2].to_string()).collect::<Vec<_>>();
        assert_eq!(refs("#12, see (#3) but not a#4, &#39; or #0 or ##5"), ["12", "3"]);
    }
}
