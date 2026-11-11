//! What each command does.

use crate::client::{encode, Client};
use crate::config::{self, Config, Host, Local};
use crate::output::{self, ago, bold, cyan, dim, green, hex, magenta, table, truncate};
use crate::{AuthCmd, BodyArgs, Cli, Command, IssueCmd, LabelCmd, MilestoneCmd, NotificationCmd, ProjectCmd, RepoCmd};
use chrono::Utc;
use reqwest::Method;
use serde::Serialize;
use serde_json::{json, Value};
use shared::{
    CurrentUser, Issue, IssueList, IssueLocation, IssueState, LabelWithCount, Milestone, NotificationList, Project,
    ProjectSummary, Repo, StateReason, TimelineItem, TokenLoginResult, ISSUE_TYPES, LOCK_REASONS, STATUS_FIELD,
};
use std::io::{IsTerminal, Read, Write};

/// What a command needs to know about where it runs.
struct Ctx {
    config: Config,
    local: Local,
    host_flag: Option<String>,
    repo_flag: Option<String>,
}

impl Ctx {
    fn host(&self) -> Result<String, String> {
        config::resolve_host(self.host_flag.as_deref(), &self.local, &self.config)
    }

    fn saved(&self) -> Option<&Host> {
        self.host().ok().and_then(|h| self.config.hosts.get(&h))
    }

    fn client(&self) -> Result<Client, String> {
        let host = self.host()?;
        // IT_TOKEN lets scripts and CI run without a config file.
        let token = std::env::var("IT_TOKEN").ok().filter(|t| !t.is_empty()).or_else(|| self.config.hosts.get(&host).map(|h| h.token.clone()));
        Ok(Client::new(host, token))
    }

    fn repo(&self) -> Result<String, String> {
        config::resolve_repo(self.repo_flag.as_deref(), &self.local, self.saved())
    }
}

fn print_json(value: &impl Serialize) -> Result<(), String> {
    println!("{}", serde_json::to_string_pretty(value).map_err(|e| e.to_string())?);
    Ok(())
}

fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

fn prompt(label: &str) -> Result<String, String> {
    eprint!("{label}");
    std::io::stderr().flush().ok();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).map_err(|e| e.to_string())?;
    Ok(line.trim().to_string())
}

fn confirm(question: &str, yes: bool) -> Result<(), String> {
    if yes {
        return Ok(());
    }
    if !interactive() {
        return Err("this cannot be undone; pass --yes to confirm".to_string());
    }
    match prompt(&format!("{question} [y/N] "))?.to_lowercase().as_str() {
        "y" | "yes" => Ok(()),
        _ => Err("cancelled".to_string()),
    }
}

fn open_in_browser(url: &str) -> Result<(), String> {
    let program = if cfg!(target_os = "macos") { "open" } else if cfg!(windows) { "explorer" } else { "xdg-open" };
    println!("Opening {url} in your browser.");
    std::process::Command::new(program).arg(url).spawn().map(|_| ()).map_err(|e| format!("could not start {program}: {e}"))
}

/// Opens `$VISUAL` / `$EDITOR` on a temporary file and returns what was saved.
fn edit_text(initial: &str) -> Result<String, String> {
    let editor = std::env::var("VISUAL").or_else(|_| std::env::var("EDITOR")).unwrap_or_else(|_| "vi".to_string());
    let path = std::env::temp_dir().join(format!("it-{}.md", std::process::id()));
    std::fs::write(&path, initial).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let mut parts = editor.split_whitespace();
    let program = parts.next().ok_or("EDITOR is empty")?;
    let status = std::process::Command::new(program).args(parts).arg(&path).status().map_err(|e| format!("could not start {editor}: {e}"));
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    if !status?.success() {
        return Err(format!("{editor} did not exit cleanly; nothing was sent"));
    }
    Ok(text.trim_end().to_string())
}

/// The body from `--body`, `--body-file`, or (in a terminal, when `ask`) the editor.
fn read_body(args: &BodyArgs, ask: bool, initial: &str) -> Result<Option<String>, String> {
    if let Some(text) = &args.body {
        return Ok(Some(text.clone()));
    }
    if let Some(path) = &args.body_file {
        let mut text = String::new();
        if path == "-" {
            std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
        } else {
            text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
        }
        return Ok(Some(text.trim_end().to_string()));
    }
    if ask && interactive() {
        return edit_text(initial).map(Some);
    }
    Ok(None)
}

fn quote(value: &str) -> String {
    if value.contains(char::is_whitespace) {
        format!("\"{value}\"")
    } else {
        value.to_string()
    }
}

fn state_mark(issue_state: IssueState, reason: Option<StateReason>, text: &str) -> String {
    match (issue_state, reason) {
        (IssueState::Open, _) => green(text),
        (IssueState::Closed, Some(StateReason::NotPlanned | StateReason::Duplicate)) => dim(text),
        (IssueState::Closed, _) => magenta(text),
    }
}

fn labels_text(issue: &Issue) -> String {
    issue.labels.iter().map(|l| hex(&l.color, &l.name)).collect::<Vec<_>>().join(", ")
}

fn issue_rows(items: &[Issue]) -> Vec<Vec<String>> {
    let now = Utc::now();
    items
        .iter()
        .map(|i| vec![state_mark(i.state, i.state_reason, &format!("#{}", i.number)), truncate(&i.title, 70), labels_text(i), dim(&ago(i.updated_at, now))])
        .collect()
}

fn parse_state(state: &str) -> Result<&'static str, String> {
    match state.to_lowercase().as_str() {
        "open" => Ok("open"),
        "closed" => Ok("closed"),
        "all" => Ok("all"),
        other => Err(format!("state must be open, closed or all, not \"{other}\"")),
    }
}

fn check_type(kind: &str) -> Result<Option<String>, String> {
    if kind.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    ISSUE_TYPES
        .iter()
        .find(|t| t.eq_ignore_ascii_case(kind))
        .map(|t| Some(t.to_string()))
        .ok_or_else(|| format!("type must be one of {} (or none), not \"{kind}\"", ISSUE_TYPES.join(", ")))
}

async fn milestone_number(client: &Client, repo: &str, title: &str) -> Result<i64, String> {
    let all: Vec<Milestone> = client.get(&format!("/repos/{repo}/milestones?state=all")).await?;
    all.iter().find(|m| m.title.eq_ignore_ascii_case(title)).map(|m| m.number).ok_or_else(|| format!("there is no milestone called \"{title}\""))
}

pub async fn run(cli: Cli) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let ctx = Ctx { config: Config::load()?, local: config::find_local(&cwd), host_flag: cli.host, repo_flag: cli.repo };
    match cli.command {
        Command::Auth(c) => auth(ctx, c).await,
        Command::Repo(c) => repo(ctx, c).await,
        Command::Issue(c) => issue(&ctx, c).await,
        Command::Label(c) => label(&ctx, c).await,
        Command::Milestone(c) => milestone(&ctx, c).await,
        Command::Project(c) => project(&ctx, c).await,
        Command::Notification(c) => notification(&ctx, c).await,
    }
}

async fn auth(mut ctx: Ctx, command: AuthCmd) -> Result<(), String> {
    match command {
        AuthCmd::Login { token, username } => {
            let host = match ctx.host() {
                Ok(h) => h,
                Err(_) if interactive() => config::normalize_host(&prompt("Server address: ")?),
                Err(e) => return Err(e),
            };
            let (user, token) = match token {
                Some(token) => {
                    let me: CurrentUser = Client::new(host.clone(), Some(token.clone())).get("/user").await?;
                    (me.login, token)
                }
                None => {
                    if !interactive() {
                        return Err("pass --token when not running in a terminal".to_string());
                    }
                    let login = match username {
                        Some(u) => u,
                        None => prompt("Username or email: ")?,
                    };
                    let password = rpassword::prompt_password("Password: ").map_err(|e| e.to_string())?;
                    let machine = std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")).unwrap_or_else(|_| "this computer".to_string());
                    let body = json!({"login": login, "password": password, "name": format!("it CLI on {machine}")});
                    let result: TokenLoginResult = Client::new(host.clone(), None).call(Method::POST, "/auth/token", &body).await?;
                    (result.login, result.token)
                }
            };
            let default_repo = ctx.config.hosts.get(&host).and_then(|h| h.default_repo.clone());
            ctx.config.hosts.insert(host.clone(), Host { token, user: user.clone(), default_repo });
            ctx.config.default_host = Some(host.clone());
            ctx.config.save()?;
            println!("{} Signed in to {host} as {}", green("✓"), bold(&user));
            Ok(())
        }
        AuthCmd::Status => {
            let host = ctx.host()?;
            let me: CurrentUser = ctx.client()?.get("/user").await?;
            println!("{host}");
            println!("  {} Signed in as {}{}", green("✓"), bold(&me.login), if me.is_admin { " (site admin)" } else { "" });
            match ctx.repo() {
                Ok(r) => println!("  Repository: {r}"),
                Err(_) => println!("  Repository: none set (use -R or `it repo set-default`)"),
            }
            println!("  Config: {}", config::path()?.display());
            Ok(())
        }
        AuthCmd::Logout => {
            let host = ctx.host()?;
            if !ctx.config.hosts.contains_key(&host) {
                return Err(format!("you are not signed in to {host}"));
            }
            // Best effort: the local copy is forgotten even when the server cannot be reached.
            let revoked = ctx.client()?.run(Method::DELETE, "/user/tokens/current", None).await;
            ctx.config.hosts.remove(&host);
            if ctx.config.default_host.as_deref() == Some(host.as_str()) {
                ctx.config.default_host = ctx.config.hosts.keys().next().cloned();
            }
            ctx.config.save()?;
            match revoked {
                Ok(()) => println!("{} Signed out of {host} and revoked the token", green("✓")),
                Err(e) => println!("Signed out of {host}. The token could not be revoked on the server ({e}); remove it in Settings → Access tokens."),
            }
            Ok(())
        }
    }
}

async fn repo(mut ctx: Ctx, command: RepoCmd) -> Result<(), String> {
    match command {
        RepoCmd::List => {
            let repos: Vec<Repo> = ctx.client()?.get("/user/repos").await?;
            if repos.is_empty() {
                println!("You have no repositories yet. Create one with `it repo create NAME`.");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = repos
                .iter()
                .map(|r| vec![bold(&format!("{}/{}", r.owner, r.name)), if r.private { "private".into() } else { "public".into() }, r.open_issues_count.to_string(), truncate(&r.description, 60)])
                .collect();
            println!("{}", table(&["NAME", "VISIBILITY", "OPEN", "DESCRIPTION"], &rows));
            Ok(())
        }
        RepoCmd::View { json, web } => {
            let name = ctx.repo()?;
            let client = ctx.client()?;
            if web {
                return open_in_browser(&format!("{}/{name}", client.host));
            }
            let r: Repo = client.get(&format!("/repos/{name}")).await?;
            if json {
                return print_json(&r);
            }
            println!("{} {}", bold(&format!("{}/{}", r.owner, r.name)), dim(if r.private { "private" } else { "public" }));
            if !r.description.is_empty() {
                println!("{}", r.description);
            }
            println!("{} open issues · your role: {}", r.open_issues_count, r.viewer_role.map_or("none".to_string(), |role| format!("{role:?}").to_lowercase()));
            println!("{}", dim(&format!("{}/{}/{}/issues", client.host, r.owner, r.name)));
            Ok(())
        }
        RepoCmd::Create { name, description, private } => {
            let client = ctx.client()?;
            let r: Repo = client.call(Method::POST, "/user/repos", &json!({"name": name, "description": description, "private": private})).await?;
            println!("{} Created {}/{}", green("✓"), r.owner, r.name);
            println!("{}/{}/{}", client.host, r.owner, r.name);
            Ok(())
        }
        RepoCmd::SetDefault { repo } => {
            let host = ctx.host()?;
            let name = config::resolve_repo(Some(&repo), &Local::default(), None)?;
            // Make sure it exists and is visible before remembering it.
            let _: Repo = ctx.client()?.get(&format!("/repos/{name}")).await?;
            let entry = ctx.config.hosts.get_mut(&host).ok_or_else(|| format!("sign in to {host} first: `it auth login`"))?;
            entry.default_repo = Some(name.clone());
            ctx.config.save()?;
            println!("{} {name} is now the default repository for {host}", green("✓"));
            Ok(())
        }
    }
}

async fn issue(ctx: &Ctx, command: IssueCmd) -> Result<(), String> {
    let name = ctx.repo()?;
    let client = ctx.client()?;
    let base = format!("/repos/{name}/issues");
    let web_base = format!("{}/{name}/issues", client.host);
    match command {
        IssueCmd::List { state, label, assignee, author, milestone, kind, search, limit, json, web } => {
            let state = parse_state(&state)?;
            let mut q = vec!["is:issue".to_string()];
            if state != "all" {
                q.push(format!("state:{state}"));
            }
            q.extend(label.iter().map(|l| format!("label:{}", quote(l))));
            if let Some(a) = &assignee {
                q.push(format!("assignee:{a}"));
            }
            if let Some(a) = &author {
                q.push(format!("author:{a}"));
            }
            if let Some(m) = &milestone {
                q.push(format!("milestone:{}", quote(m)));
            }
            if let Some(k) = &kind {
                q.push(format!("type:{}", check_type(k)?.unwrap_or_default()));
            }
            if let Some(s) = &search {
                q.push(s.clone());
            }
            let q = q.join(" ");
            if web {
                return open_in_browser(&format!("{web_base}?q={}", encode(&q)));
            }
            let per_page = limit.clamp(1, 100);
            let list: IssueList = client.get(&format!("{base}?q={}&per_page={per_page}", encode(&q))).await?;
            if json {
                return print_json(&list.items);
            }
            if list.items.is_empty() {
                println!("No issues match your search in {name}");
                return Ok(());
            }
            let what = if state == "all" { "issues".to_string() } else { format!("{state} issues") };
            println!("\nShowing {} of {} {what} in {name}\n", list.items.len(), list.total_count);
            println!("{}", table(&["ID", "TITLE", "LABELS", "UPDATED"], &issue_rows(&list.items)));
            Ok(())
        }
        IssueCmd::View { number, comments, json, web } => {
            if web {
                return open_in_browser(&format!("{web_base}/{number}"));
            }
            let i: Issue = client.get(&format!("{base}/{number}")).await?;
            let timeline: Vec<TimelineItem> = if comments { client.get(&format!("{base}/{number}/timeline")).await? } else { Vec::new() };
            if json {
                return if comments { print_json(&json!({"issue": i, "timeline": timeline})) } else { print_json(&i) };
            }
            let now = Utc::now();
            let state = if i.state == IssueState::Open { "Open" } else { "Closed" };
            println!("{} {}", bold(&i.title), dim(&format!("#{}", i.number)));
            println!(
                "{} • {} opened {} • {} comment{}",
                state_mark(i.state, i.state_reason, state),
                i.author.login,
                ago(i.created_at, now),
                i.comments,
                if i.comments == 1 { "" } else { "s" }
            );
            let line = |label: &str, value: String| {
                if !value.is_empty() {
                    println!("{} {value}", bold(&format!("{label}:")));
                }
            };
            line("Assignees", i.assignees.iter().map(|a| a.login.clone()).collect::<Vec<_>>().join(", "));
            line("Labels", labels_text(&i));
            line("Milestone", i.milestone.as_ref().map(|m| m.title.clone()).unwrap_or_default());
            line("Type", i.issue_type.clone().unwrap_or_default());
            line("Parent", i.parent.as_ref().map(|p| format!("#{} {}", p.number, p.title)).unwrap_or_default());
            line("Sub-issues", i.sub_issues.iter().map(|s| format!("#{}", s.number)).collect::<Vec<_>>().join(", "));
            line("Blocked by", i.blocked_by.iter().map(|s| format!("#{}", s.number)).collect::<Vec<_>>().join(", "));
            line("Blocking", i.blocking.iter().map(|s| format!("#{}", s.number)).collect::<Vec<_>>().join(", "));
            if i.locked {
                println!("{}", dim("This conversation is locked."));
            }
            println!();
            print_markdown(if i.body.trim().is_empty() { "_No description provided._" } else { &i.body });
            for item in &timeline {
                if let TimelineItem::Comment(c) = item {
                    println!("\n{}", dim(&"─".repeat(40)));
                    println!("{} commented {}", bold(&c.author.login), dim(&ago(c.created_at, now)));
                    println!();
                    print_markdown(&c.body);
                }
            }
            println!("\n{}", dim(&format!("View this issue: {web_base}/{number}")));
            Ok(())
        }
        IssueCmd::Create { title, body, label, assignee, milestone, kind, web } => {
            if web {
                return open_in_browser(&format!("{web_base}/new"));
            }
            let title = match title {
                Some(t) => t,
                None if interactive() => prompt("Title: ")?,
                None => return Err("pass --title when not running in a terminal".to_string()),
            };
            if title.trim().is_empty() {
                return Err("the title cannot be blank".to_string());
            }
            let text = read_body(&body, true, "")?.unwrap_or_default();
            let mut request = json!({"title": title, "body": text, "labels": label, "assignees": assignee});
            if let Some(m) = &milestone {
                request["milestone"] = json!(milestone_number(&client, &name, m).await?);
            }
            if let Some(k) = &kind {
                request["issue_type"] = json!(check_type(k)?);
            }
            let made: Issue = client.call(Method::POST, &base, &request).await?;
            // Without triage access the server drops organising fields; say so instead of pretending.
            let dropped = (!label.is_empty() && made.labels.is_empty()) || (!assignee.is_empty() && made.assignees.is_empty()) || (milestone.is_some() && made.milestone.is_none());
            println!("{web_base}/{}", made.number);
            if dropped {
                eprintln!("note: labels, assignees and milestone were not applied; that needs triage access to {name}");
            }
            Ok(())
        }
        IssueCmd::Edit { number, title, body, add_label, remove_label, add_assignee, remove_assignee, milestone, remove_milestone, kind } => {
            let mut request = serde_json::Map::new();
            if let Some(t) = title {
                request.insert("title".into(), json!(t));
            }
            if let Some(text) = read_body(&body, false, "")? {
                request.insert("body".into(), json!(text));
            }
            let needs_current = !(add_label.is_empty() && remove_label.is_empty() && add_assignee.is_empty() && remove_assignee.is_empty());
            if needs_current {
                let current: Issue = client.get(&format!("{base}/{number}")).await?;
                let merge = |have: Vec<String>, add: &[String], remove: &[String]| {
                    let mut out: Vec<String> = have.into_iter().filter(|h| !remove.iter().any(|r| r.eq_ignore_ascii_case(h))).collect();
                    for a in add {
                        if !out.iter().any(|o| o.eq_ignore_ascii_case(a)) {
                            out.push(a.clone());
                        }
                    }
                    out
                };
                if !(add_label.is_empty() && remove_label.is_empty()) {
                    request.insert("labels".into(), json!(merge(current.labels.iter().map(|l| l.name.clone()).collect(), &add_label, &remove_label)));
                }
                if !(add_assignee.is_empty() && remove_assignee.is_empty()) {
                    request.insert("assignees".into(), json!(merge(current.assignees.iter().map(|a| a.login.clone()).collect(), &add_assignee, &remove_assignee)));
                }
            }
            if let Some(m) = &milestone {
                request.insert("milestone".into(), json!(milestone_number(&client, &name, m).await?));
            } else if remove_milestone {
                request.insert("milestone".into(), Value::Null);
            }
            if let Some(k) = &kind {
                request.insert("issue_type".into(), json!(check_type(k)?));
            }
            if request.is_empty() {
                return Err("nothing to change; see `it issue edit --help`".to_string());
            }
            let _: Issue = client.call(Method::PATCH, &format!("{base}/{number}"), &Value::Object(request)).await?;
            println!("{web_base}/{number}");
            Ok(())
        }
        IssueCmd::Close { number, reason, comment } => {
            let reason = match reason.to_lowercase().replace(['-', '_'], " ").as_str() {
                "completed" => "completed",
                "not planned" => "not_planned",
                "duplicate" => "duplicate",
                other => return Err(format!("reason must be completed, \"not planned\" or duplicate, not \"{other}\"")),
            };
            if let Some(text) = comment.filter(|c| !c.trim().is_empty()) {
                client.run(Method::POST, &format!("{base}/{number}/comments"), Some(json!({"body": text}))).await?;
            }
            let i: Issue = client.call(Method::PATCH, &format!("{base}/{number}"), &json!({"state": "closed", "state_reason": reason})).await?;
            println!("{} Closed issue #{} ({})", magenta("✓"), i.number, i.title);
            Ok(())
        }
        IssueCmd::Reopen { number, comment } => {
            if let Some(text) = comment.filter(|c| !c.trim().is_empty()) {
                client.run(Method::POST, &format!("{base}/{number}/comments"), Some(json!({"body": text}))).await?;
            }
            let i: Issue = client.call(Method::PATCH, &format!("{base}/{number}"), &json!({"state": "open"})).await?;
            println!("{} Reopened issue #{} ({})", green("✓"), i.number, i.title);
            Ok(())
        }
        IssueCmd::Comment { number, body } => {
            let text = read_body(&body, true, "")?.ok_or("pass --body or --body-file when not running in a terminal")?;
            if text.trim().is_empty() {
                return Err("the comment is empty; nothing was sent".to_string());
            }
            let made: Value = client.call(Method::POST, &format!("{base}/{number}/comments"), &json!({"body": text})).await?;
            println!("{web_base}/{number}#issuecomment-{}", made["id"].as_str().unwrap_or_default());
            Ok(())
        }
        IssueCmd::Delete { number, yes } => {
            let i: Issue = client.get(&format!("{base}/{number}")).await?;
            confirm(&format!("Delete issue #{} \"{}\" with its comments? This cannot be undone.", i.number, i.title), yes)?;
            client.run(Method::DELETE, &format!("{base}/{number}"), None).await?;
            println!("{} Deleted issue #{number}", green("✓"));
            Ok(())
        }
        IssueCmd::Pin { number } => {
            client.run(Method::PUT, &format!("{base}/{number}/pin"), None).await?;
            println!("{} Pinned issue #{number}", green("✓"));
            Ok(())
        }
        IssueCmd::Block { number, by } => {
            client.run(Method::POST, &format!("{base}/{number}/dependencies/blocked_by"), Some(json!({"number": by}))).await?;
            println!("{} Issue #{number} is now blocked by #{by}", green("✓"));
            Ok(())
        }
        IssueCmd::Unblock { number, by } => {
            client.run(Method::DELETE, &format!("{base}/{number}/dependencies/blocked_by/{by}"), None).await?;
            println!("{} Issue #{number} is no longer blocked by #{by}", green("✓"));
            Ok(())
        }
        IssueCmd::Unpin { number } => {
            client.run(Method::DELETE, &format!("{base}/{number}/pin"), None).await?;
            println!("{} Unpinned issue #{number}", green("✓"));
            Ok(())
        }
        IssueCmd::Lock { number, reason } => {
            let reason = match reason {
                None => None,
                Some(r) => Some(LOCK_REASONS.iter().find(|k| k.eq_ignore_ascii_case(&r.replace('_', " "))).ok_or_else(|| format!("reason must be one of: {}", LOCK_REASONS.join(", ")))?.to_string()),
            };
            client.run(Method::PUT, &format!("{base}/{number}/lock"), Some(json!({"lock_reason": reason}))).await?;
            println!("{} Locked issue #{number}", green("✓"));
            Ok(())
        }
        IssueCmd::Unlock { number } => {
            client.run(Method::DELETE, &format!("{base}/{number}/lock"), None).await?;
            println!("{} Unlocked issue #{number}", green("✓"));
            Ok(())
        }
        IssueCmd::Transfer { number, destination } => {
            let moved: IssueLocation = client.call(Method::POST, &format!("{base}/{number}/transfer"), &json!({"repo": destination})).await?;
            println!("{}/{}/{}/issues/{}", client.host, moved.owner, moved.repo, moved.number);
            Ok(())
        }
        IssueCmd::Status => {
            for (heading, qualifier) in [("Issues assigned to you", "assignee:@me"), ("Issues opened by you", "author:@me")] {
                let q = format!("is:issue state:open {qualifier}");
                let list: IssueList = client.get(&format!("{base}?q={}&per_page=10", encode(&q))).await?;
                println!("\n{}", bold(heading));
                if list.items.is_empty() {
                    println!("  There are no open issues {}", if qualifier.starts_with("assignee") { "assigned to you" } else { "opened by you" });
                } else {
                    for row in table(&["ID", "TITLE", "LABELS", "UPDATED"], &issue_rows(&list.items)).lines() {
                        println!("  {row}");
                    }
                }
            }
            println!();
            Ok(())
        }
    }
}

/// Markdown formatted for a terminal; plain text when piped.
fn print_markdown(text: &str) {
    if output::colors() {
        termimad::MadSkin::default().print_text(text);
    } else {
        println!("{text}");
    }
}

async fn label(ctx: &Ctx, command: LabelCmd) -> Result<(), String> {
    let name = ctx.repo()?;
    let client = ctx.client()?;
    let base = format!("/repos/{name}/labels");
    match command {
        LabelCmd::List { json } => {
            let labels: Vec<LabelWithCount> = client.get(&base).await?;
            if json {
                return print_json(&labels);
            }
            println!("\nShowing {} labels in {name}\n", labels.len());
            let rows: Vec<Vec<String>> = labels
                .iter()
                .map(|l| vec![hex(&l.label.color, &l.label.name), truncate(&l.label.description, 60), format!("#{}", l.label.color), l.open_issues.to_string()])
                .collect();
            println!("{}", table(&["NAME", "DESCRIPTION", "COLOR", "OPEN"], &rows));
            Ok(())
        }
        LabelCmd::Create { name: label_name, color, description } => {
            client.run(Method::POST, &base, Some(json!({"name": label_name, "color": color, "description": description}))).await?;
            println!("{} Label \"{label_name}\" created in {name}", green("✓"));
            Ok(())
        }
        LabelCmd::Edit { name: label_name, name_new, color, description } => {
            if name_new.is_none() && color.is_none() && description.is_none() {
                return Err("nothing to change; pass --name-new, --color or --description".to_string());
            }
            let body = json!({"new_name": name_new, "color": color, "description": description});
            client.run(Method::PATCH, &format!("{base}/{}", encode(&label_name)), Some(body)).await?;
            println!("{} Label \"{label_name}\" updated", green("✓"));
            Ok(())
        }
        LabelCmd::Delete { name: label_name, yes } => {
            confirm(&format!("Delete label \"{label_name}\" from {name}? It is removed from every issue."), yes)?;
            client.run(Method::DELETE, &format!("{base}/{}", encode(&label_name)), None).await?;
            println!("{} Label \"{label_name}\" deleted", green("✓"));
            Ok(())
        }
    }
}

async fn milestone(ctx: &Ctx, command: MilestoneCmd) -> Result<(), String> {
    let name = ctx.repo()?;
    let client = ctx.client()?;
    let base = format!("/repos/{name}/milestones");
    match command {
        MilestoneCmd::List { state, json } => {
            let state = parse_state(&state)?;
            let list: Vec<Milestone> = client.get(&format!("{base}?state={state}")).await?;
            if json {
                return print_json(&list);
            }
            if list.is_empty() {
                println!("No {state} milestones in {name}");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = list
                .iter()
                .map(|m| {
                    let total = m.open_issues + m.closed_issues;
                    let percent = (m.closed_issues * 100).checked_div(total).unwrap_or(0);
                    vec![
                        format!("#{}", m.number),
                        bold(&truncate(&m.title, 40)),
                        m.due_on.map_or("no due date".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                        format!("{percent}% ({} open, {} closed)", m.open_issues, m.closed_issues),
                    ]
                })
                .collect();
            println!("{}", table(&["ID", "TITLE", "DUE", "PROGRESS"], &rows));
            Ok(())
        }
        MilestoneCmd::Create { title, description, due } => {
            let due_on = match due {
                None => None,
                Some(d) => {
                    chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").map_err(|_| format!("\"{d}\" is not a date; use YYYY-MM-DD"))?;
                    Some(format!("{d}T00:00:00Z"))
                }
            };
            let made: Milestone = client.call(Method::POST, &base, &json!({"title": title, "description": description, "due_on": due_on})).await?;
            println!("{} Milestone #{} \"{}\" created in {name}", green("✓"), made.number, made.title);
            Ok(())
        }
        MilestoneCmd::Close { number } | MilestoneCmd::Reopen { number } => {
            let closing = matches!(command, MilestoneCmd::Close { .. });
            let m: Milestone = client.call(Method::PATCH, &format!("{base}/{number}"), &json!({"state": if closing { "closed" } else { "open" }})).await?;
            println!("{} Milestone \"{}\" {}", green("✓"), m.title, if closing { "closed" } else { "reopened" });
            Ok(())
        }
        MilestoneCmd::Delete { number, yes } => {
            let m: Milestone = client.get(&format!("{base}/{number}")).await?;
            confirm(&format!("Delete milestone \"{}\"? Its issues are kept.", m.title), yes)?;
            client.run(Method::DELETE, &format!("{base}/{number}"), None).await?;
            println!("{} Milestone \"{}\" deleted", green("✓"), m.title);
            Ok(())
        }
    }
}

async fn project(ctx: &Ctx, command: ProjectCmd) -> Result<(), String> {
    let name = ctx.repo()?;
    let client = ctx.client()?;
    let base = format!("/repos/{name}/projects");
    match command {
        ProjectCmd::List { json } => {
            let list: Vec<ProjectSummary> = client.get(&base).await?;
            if json {
                return print_json(&list);
            }
            if list.is_empty() {
                println!("No projects in {name}");
                return Ok(());
            }
            let now = Utc::now();
            let rows: Vec<Vec<String>> = list
                .iter()
                .map(|p| vec![format!("#{}", p.number), bold(&truncate(&p.title, 50)), if p.closed { "closed".into() } else { "open".into() }, p.item_count.to_string(), dim(&ago(p.updated_at, now))])
                .collect();
            println!("{}", table(&["ID", "TITLE", "STATE", "ITEMS", "UPDATED"], &rows));
            Ok(())
        }
        ProjectCmd::View { number, json, web } => {
            if web {
                return open_in_browser(&format!("{}/{name}/projects/{number}", client.host));
            }
            let p: Project = client.get(&format!("{base}/{number}")).await?;
            if json {
                return print_json(&p);
            }
            println!("{} {}", bold(&p.summary.title), dim(&format!("#{} · {} items", p.summary.number, p.summary.item_count)));
            let status = p.fields.iter().find(|f| f.id == STATUS_FIELD);
            let mut columns: Vec<(String, Option<String>)> = vec![("No Status".to_string(), None)];
            columns.extend(status.iter().flat_map(|f| f.options.iter().map(|o| (o.name.clone(), Some(o.id.clone())))));
            for (title, id) in columns {
                let cards: Vec<_> = p.items.iter().filter(|i| i.values.get(STATUS_FIELD).and_then(Value::as_str).map(str::to_string) == id).collect();
                if cards.is_empty() && id.is_none() {
                    continue;
                }
                println!("\n{} {}", cyan(&title), dim(&format!("({})", cards.len())));
                for c in cards {
                    println!("  {} {}", state_mark(c.issue.state, c.issue.state_reason, &format!("#{}", c.issue.number)), truncate(&c.issue.title, 80));
                }
            }
            Ok(())
        }
        ProjectCmd::ItemAdd { number, issue } => {
            client.run(Method::POST, &format!("{base}/{number}/items"), Some(json!({"issue_number": issue}))).await?;
            println!("{} Added issue #{issue} to project #{number}", green("✓"));
            Ok(())
        }
        ProjectCmd::ItemStatus { number, issue, status } => {
            let p: Project = client.get(&format!("{base}/{number}")).await?;
            let item = p.items.iter().find(|i| i.issue.number == issue).ok_or_else(|| format!("issue #{issue} is not in project #{number}; add it with `it project item-add {number} {issue}`"))?;
            let options = p.fields.iter().find(|f| f.id == STATUS_FIELD).map(|f| f.options.clone()).unwrap_or_default();
            let value = if status.eq_ignore_ascii_case("none") {
                Value::Null
            } else {
                let option = options.iter().find(|o| o.name.eq_ignore_ascii_case(&status)).ok_or_else(|| {
                    format!("there is no status \"{status}\"; choose one of: {}, none", options.iter().map(|o| o.name.clone()).collect::<Vec<_>>().join(", "))
                })?;
                json!(option.id)
            };
            client.run(Method::PATCH, &format!("{base}/{number}/items/{}", item.id), Some(json!({"values": {STATUS_FIELD: value}}))).await?;
            println!("{} Issue #{issue} is now {status} in project #{number}", green("✓"));
            Ok(())
        }
    }
}

async fn notification(ctx: &Ctx, command: NotificationCmd) -> Result<(), String> {
    let client = ctx.client()?;
    match command {
        NotificationCmd::List { filter, json } => {
            if !["inbox", "unread", "done"].contains(&filter.as_str()) {
                return Err(format!("filter must be inbox, unread or done, not \"{filter}\""));
            }
            let list: NotificationList = client.get(&format!("/notifications?filter={filter}")).await?;
            if json {
                return print_json(&list.items);
            }
            if list.items.is_empty() {
                println!("{}", if filter == "done" { "Nothing marked as done." } else { "All caught up!" });
                return Ok(());
            }
            println!("\n{} unread · showing {} of {}\n", list.unread_count, list.items.len(), list.total_count);
            let now = Utc::now();
            let rows: Vec<Vec<String>> = list
                .items
                .iter()
                .map(|n| {
                    vec![
                        if n.unread { cyan("●") } else { " ".into() },
                        format!("{}#{}", n.repo, n.issue.number),
                        truncate(&n.issue.title, 55),
                        n.reason.replace('_', " "),
                        dim(&ago(n.updated_at, now)),
                    ]
                })
                .collect();
            println!("{}", table(&["", "ISSUE", "TITLE", "REASON", "UPDATED"], &rows));
            Ok(())
        }
        NotificationCmd::Read => {
            client.run(Method::POST, "/notifications/read", None).await?;
            println!("{} All notifications marked as read", green("✓"));
            Ok(())
        }
    }
}
