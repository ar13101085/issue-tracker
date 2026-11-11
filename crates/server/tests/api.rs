mod common;

use actix_web::http::{Method, StatusCode};
use common::{drop_db, send, signup, state};
use serde_json::json;

const GET: Method = Method::GET;
const POST: Method = Method::POST;
const PATCH: Method = Method::PATCH;
const PUT: Method = Method::PUT;
const DELETE: Method = Method::DELETE;

#[actix_web::test]
async fn accounts_and_sessions() {
    let st = state().await;
    let app = app!(st);

    let root = signup(&app, "root").await;
    let me = send(&app, GET, "/api/user", Some(&root), None).await;
    assert_eq!(me.json["login"], "root");
    assert_eq!(me.json["is_admin"], true, "the first account is the admin");

    let alice = signup(&app, "Alice").await;
    let me = send(&app, GET, "/api/user", Some(&alice), None).await;
    assert_eq!(me.json["is_admin"], false);

    // Same name in another case, bad names, short password.
    for (login, email, password, field) in [
        ("alice", "a2@example.test", "correct horse", "login"),
        ("-bad", "b@example.test", "correct horse", "login"),
        ("a--b", "c@example.test", "correct horse", "login"),
        ("login", "d@example.test", "correct horse", "login"),
        ("dave", "not-an-email", "correct horse", "email"),
        ("erin", "alice@example.test", "correct horse", "email"),
        ("frank", "f@example.test", "short", "password"),
    ] {
        let body = json!({"login": login, "email": email, "password": password});
        let r = send(&app, POST, "/api/auth/signup", None, Some(body)).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{login}");
        assert_eq!(r.json["errors"][0]["field"], field, "{login}: {}", r.json);
    }

    let bad = send(&app, POST, "/api/auth/login", None, Some(json!({"login": "alice", "password": "wrong"}))).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    assert!(bad.cookie.is_none());

    // Login by name (any case) and by email.
    for id in ["ALICE", "alice@example.test"] {
        let ok = send(&app, POST, "/api/auth/login", None, Some(json!({"login": id, "password": "correct horse"}))).await;
        assert_eq!(ok.status, StatusCode::OK, "{id}");
        assert_eq!(ok.json["login"], "Alice");
    }

    assert_eq!(send(&app, GET, "/api/user", None, None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(send(&app, GET, "/api/user", Some("made-up"), None).await.status, StatusCode::UNAUTHORIZED);

    assert_eq!(send(&app, POST, "/api/auth/logout", Some(&alice), None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(send(&app, GET, "/api/user", Some(&alice), None).await.status, StatusCode::UNAUTHORIZED);

    drop_db(&st).await;
}

#[actix_web::test]
async fn private_repositories_and_roles() {
    let st = state().await;
    let app = app!(st);
    let _root = signup(&app, "root").await;
    let alice = signup(&app, "alice").await;
    let bob = signup(&app, "bob").await;
    let carol = signup(&app, "carol").await;

    let made = send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "secret", "private": true}))).await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.json);
    assert_eq!(made.json["viewer_role"], "admin");
    let dup = send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "Secret"}))).await;
    assert_eq!(dup.status, StatusCode::UNPROCESSABLE_ENTITY);
    let bad = send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "has space"}))).await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(send(&app, POST, "/api/user/repos", None, Some(json!({"name": "x"}))).await.status, StatusCode::UNAUTHORIZED);

    let repo = "/api/repos/alice/secret";
    let issue = send(&app, POST, &format!("{repo}/issues"), Some(&alice), Some(json!({"title": "Hidden bug"}))).await;
    assert_eq!(issue.status, StatusCode::CREATED);

    // Outsiders cannot tell the repository exists.
    for who in [None, Some(bob.as_str())] {
        assert_eq!(send(&app, GET, repo, who, None).await.status, StatusCode::NOT_FOUND);
        assert_eq!(send(&app, GET, &format!("{repo}/issues"), who, None).await.status, StatusCode::NOT_FOUND);
        assert_eq!(send(&app, GET, &format!("{repo}/issues/1"), who, None).await.status, StatusCode::NOT_FOUND);
    }
    let listed = send(&app, GET, "/api/users/alice/repos", Some(&bob), None).await;
    assert_eq!(listed.json.as_array().unwrap().len(), 0);

    // Only an admin manages collaborators.
    let put = |role: &str| json!({"role": role});
    let denied = send(&app, PUT, &format!("{repo}/collaborators/bob"), Some(&carol), Some(put("admin"))).await;
    assert_eq!(denied.status, StatusCode::NOT_FOUND);
    let added = send(&app, PUT, &format!("{repo}/collaborators/bob"), Some(&alice), Some(put("triage"))).await;
    assert_eq!(added.status, StatusCode::OK, "{}", added.json);
    let unknown = send(&app, PUT, &format!("{repo}/collaborators/nobody"), Some(&alice), Some(put("read"))).await;
    assert_eq!(unknown.status, StatusCode::UNPROCESSABLE_ENTITY);

    let seen = send(&app, GET, repo, Some(&bob), None).await;
    assert_eq!(seen.json["viewer_role"], "triage");
    assert_eq!(send(&app, GET, "/api/user/repos", Some(&bob), None).await.json.as_array().unwrap().len(), 1);
    assert_eq!(send(&app, GET, repo, Some(&carol), None).await.status, StatusCode::NOT_FOUND);

    // Triage closes issues but does not edit them or touch settings.
    let closed = send(&app, PATCH, &format!("{repo}/issues/1"), Some(&bob), Some(json!({"state": "closed"}))).await;
    assert_eq!(closed.json["state"], "closed", "{}", closed.json);
    let edit = send(&app, PATCH, &format!("{repo}/issues/1"), Some(&bob), Some(json!({"title": "x"}))).await;
    assert_eq!(edit.status, StatusCode::FORBIDDEN);
    for (method, path) in [(GET, format!("{repo}/collaborators")), (DELETE, repo.to_string()), (DELETE, format!("{repo}/issues/1"))] {
        assert_eq!(send(&app, method, &path, Some(&bob), None).await.status, StatusCode::FORBIDDEN, "{path}");
    }
    let patch = send(&app, PATCH, repo, Some(&bob), Some(json!({"private": false}))).await;
    assert_eq!(patch.status, StatusCode::FORBIDDEN);

    // Write edits.
    send(&app, PUT, &format!("{repo}/collaborators/bob"), Some(&alice), Some(put("write"))).await;
    let edit = send(&app, PATCH, &format!("{repo}/issues/1"), Some(&bob), Some(json!({"title": "Renamed"}))).await;
    assert_eq!(edit.json["title"], "Renamed");

    // Removed collaborators lose access again.
    let gone = send(&app, DELETE, &format!("{repo}/collaborators/bob"), Some(&alice), None).await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT);
    assert_eq!(send(&app, GET, repo, Some(&bob), None).await.status, StatusCode::NOT_FOUND);

    // Deleting the repository removes its issues.
    assert_eq!(send(&app, DELETE, repo, Some(&alice), None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(send(&app, GET, repo, Some(&alice), None).await.status, StatusCode::NOT_FOUND);
    assert_eq!(st.db.issues.count_documents(mongodb::bson::doc! {}).await.unwrap(), 0);

    drop_db(&st).await;
}

#[actix_web::test]
async fn public_repository_permissions() {
    let st = state().await;
    let app = app!(st);
    let root = signup(&app, "root").await;
    let alice = signup(&app, "alice").await;
    let bob = signup(&app, "bob").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "open", "description": "Public one"}))).await;
    let repo = "/api/repos/alice/open";

    // Anyone reads, nobody anonymous writes.
    let seen = send(&app, GET, repo, None, None).await;
    assert_eq!(seen.status, StatusCode::OK);
    assert!(seen.json["viewer_role"].is_null());
    assert_eq!(send(&app, GET, &format!("{repo}/issues"), None, None).await.status, StatusCode::OK);
    let anon = send(&app, POST, &format!("{repo}/issues"), None, Some(json!({"title": "x"}))).await;
    assert_eq!(anon.status, StatusCode::UNAUTHORIZED);

    // A signed-in stranger has read: opens issues, manages only their own.
    assert_eq!(send(&app, GET, repo, Some(&bob), None).await.json["viewer_role"], "read");
    let mine = send(&app, POST, &format!("{repo}/issues"), Some(&bob), Some(json!({"title": "Bob's", "body": "text"}))).await;
    assert_eq!(mine.status, StatusCode::CREATED);
    assert_eq!(mine.json["number"], 1);
    assert_eq!(mine.json["viewer_can_edit"], true);
    send(&app, POST, &format!("{repo}/issues"), Some(&alice), Some(json!({"title": "Alice's"}))).await;
    let other = send(&app, GET, &format!("{repo}/issues/2"), Some(&bob), None).await;
    assert_eq!(other.json["viewer_can_edit"], false);
    assert_eq!(other.json["viewer_can_close"], false);
    let close = send(&app, PATCH, &format!("{repo}/issues/2"), Some(&bob), Some(json!({"state": "closed"}))).await;
    assert_eq!(close.status, StatusCode::FORBIDDEN);
    let own = send(&app, PATCH, &format!("{repo}/issues/1"), Some(&bob), Some(json!({"state": "closed", "title": "Bob's own"}))).await;
    assert_eq!(own.json["state"], "closed");
    assert_eq!(own.json["title"], "Bob's own");

    // The site admin is admin everywhere.
    assert_eq!(send(&app, GET, repo, Some(&root), None).await.json["viewer_role"], "admin");

    // Going private hides it from the stranger.
    let hidden = send(&app, PATCH, repo, Some(&alice), Some(json!({"private": true}))).await;
    assert_eq!(hidden.json["private"], true);
    assert_eq!(send(&app, GET, repo, Some(&bob), None).await.status, StatusCode::NOT_FOUND);

    drop_db(&st).await;
}

#[actix_web::test]
async fn issue_lifecycle_and_listing() {
    let st = state().await;
    let app = app!(st);
    let alice = signup(&app, "alice").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "app"}))).await;
    let issues = "/api/repos/alice/app/issues";

    for (field, body) in [("title", json!({"title": "   "})), ("title", json!({"title": "x".repeat(257)}))] {
        let r = send(&app, POST, issues, Some(&alice), Some(body)).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(r.json["errors"][0]["field"], field);
    }

    for (i, title) in ["Login fails", "Crash on start", "Typo in footer", "Add dark mode", "Slow search (v2)"].iter().enumerate() {
        let r = send(&app, POST, issues, Some(&alice), Some(json!({"title": title, "body": format!("Body {i}")}))).await;
        assert_eq!(r.json["number"], i as i64 + 1, "numbers are sequential");
        assert_eq!(r.json["state"], "open");
    }

    let closed = send(&app, PATCH, &format!("{issues}/2"), Some(&alice), Some(json!({"state": "closed"}))).await;
    assert_eq!(closed.json["state_reason"], "completed");
    assert!(closed.json["closed_at"].is_string());
    let np = json!({"state": "closed", "state_reason": "not_planned"});
    let closed = send(&app, PATCH, &format!("{issues}/3"), Some(&alice), Some(np)).await;
    assert_eq!(closed.json["state_reason"], "not_planned");

    let numbers = |v: &serde_json::Value| -> Vec<i64> {
        v["items"].as_array().unwrap().iter().map(|i| i["number"].as_i64().unwrap()).collect()
    };
    let open = send(&app, GET, issues, None, None).await;
    assert_eq!((open.json["open_count"].as_u64(), open.json["closed_count"].as_u64()), (Some(3), Some(2)));
    assert_eq!(open.json["total_count"], 3);
    assert_eq!(numbers(&open.json), [5, 4, 1], "newest first");
    assert_eq!(numbers(&send(&app, GET, &format!("{issues}?state=closed"), None, None).await.json), [3, 2]);
    assert_eq!(numbers(&send(&app, GET, &format!("{issues}?state=all&direction=asc"), None, None).await.json), [1, 2, 3, 4, 5]);

    // Paging.
    let p1 = send(&app, GET, &format!("{issues}?state=all&per_page=2"), None, None).await;
    assert_eq!(numbers(&p1.json), [5, 4]);
    assert_eq!(p1.json["total_count"], 5);
    assert_eq!(numbers(&send(&app, GET, &format!("{issues}?state=all&per_page=2&page=3"), None, None).await.json), [1]);
    assert_eq!(numbers(&send(&app, GET, &format!("{issues}?state=all&per_page=2&page=9"), None, None).await.json), [] as [i64; 0]);

    // Text search is literal and case-insensitive; counts follow it.
    let found = send(&app, GET, &format!("{issues}?state=all&q=CRASH"), None, None).await;
    assert_eq!(numbers(&found.json), [2]);
    assert_eq!((found.json["open_count"].as_u64(), found.json["closed_count"].as_u64()), (Some(0), Some(1)));
    assert_eq!(numbers(&send(&app, GET, &format!("{issues}?q=%28v2%29"), None, None).await.json), [5]);
    assert_eq!(numbers(&send(&app, GET, &format!("{issues}?q=.%2A"), None, None).await.json), [] as [i64; 0]);
    assert_eq!(numbers(&send(&app, GET, &format!("{issues}?state=all&q=body+3"), None, None).await.json), [4]);

    assert_eq!(send(&app, GET, &format!("{issues}?state=nope"), None, None).await.status, StatusCode::BAD_REQUEST);
    assert_eq!(send(&app, GET, &format!("{issues}?page=x"), None, None).await.status, StatusCode::BAD_REQUEST);

    // Updated sort follows edits.
    send(&app, PATCH, &format!("{issues}/1"), Some(&alice), Some(json!({"body": "edited"}))).await;
    assert_eq!(numbers(&send(&app, GET, &format!("{issues}?sort=updated"), None, None).await.json)[0], 1);

    let reopened = send(&app, PATCH, &format!("{issues}/2"), Some(&alice), Some(json!({"state": "open"}))).await;
    assert_eq!(reopened.json["state"], "open");
    assert_eq!(reopened.json["state_reason"], "reopened");
    assert!(reopened.json["closed_at"].is_null());
    assert_eq!(send(&app, GET, "/api/repos/alice/app", None, None).await.json["open_issues_count"], 4);

    assert_eq!(send(&app, DELETE, &format!("{issues}/5"), Some(&alice), None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(send(&app, GET, &format!("{issues}/5"), None, None).await.status, StatusCode::NOT_FOUND);
    // A deleted number is not reused.
    let next = send(&app, POST, issues, Some(&alice), Some(json!({"title": "After delete"}))).await;
    assert_eq!(next.json["number"], 6);

    drop_db(&st).await;
}

#[actix_web::test]
async fn api_edges() {
    let st = state().await;
    let app = app!(st);
    let alice = signup(&app, "alice").await;

    let missing = send(&app, GET, "/api/nope", None, None).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.json["message"], "Not Found");

    // A write carrying our cookie from another site is refused.
    let req = actix_web::test::TestRequest::post()
        .uri("/api/user/repos")
        .insert_header(("cookie", format!("it_session={alice}")))
        .insert_header(("origin", "https://evil.example"))
        .insert_header(("host", "tracker.example"))
        .set_json(json!({"name": "x"}))
        .to_request();
    assert_eq!(actix_web::test::call_service(&app, req).await.status(), StatusCode::FORBIDDEN);
    let req = actix_web::test::TestRequest::post()
        .uri("/api/user/repos")
        .insert_header(("cookie", format!("it_session={alice}")))
        .insert_header(("origin", "https://tracker.example"))
        .insert_header(("host", "tracker.example"))
        .set_json(json!({"name": "x"}))
        .to_request();
    assert_eq!(actix_web::test::call_service(&app, req).await.status(), StatusCode::CREATED);

    let req = actix_web::test::TestRequest::post()
        .uri("/api/auth/login")
        .insert_header(("content-type", "application/json"))
        .set_payload("{not json")
        .to_request();
    assert_eq!(actix_web::test::call_service(&app, req).await.status(), StatusCode::BAD_REQUEST);

    drop_db(&st).await;
}

#[actix_web::test]
async fn comments_timeline_and_markdown() {
    let st = state().await;
    let app = app!(st);
    let alice = signup(&app, "alice").await;
    let bob = signup(&app, "bob").await;
    let carol = signup(&app, "carol").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "app"}))).await;
    let repo = "/api/repos/alice/app";
    let issues = format!("{repo}/issues");

    let first = send(&app, POST, &issues, Some(&alice), Some(json!({"title": "First", "body": "Hello **world**\n<script>alert(1)</script>"}))).await;
    assert!(first.json["body_html"].as_str().unwrap().contains("<strong>world</strong>"));
    assert!(!first.json["body_html"].as_str().unwrap().contains("<script"));
    assert_eq!(first.json["author_association"], "owner");

    // #1 and @bob resolve; #99 and @nobody stay text. The reference lands on issue 1 once.
    let second = send(&app, POST, &issues, Some(&bob), Some(json!({"title": "Second", "body": "Same as #1, ask @bob or @nobody about #99"}))).await;
    let html = second.json["body_html"].as_str().unwrap();
    assert!(html.contains(r##"<a class="issue-link" href="/alice/app/issues/1">#1</a>"##), "{html}");
    assert!(html.contains(r#"<a class="user-mention" href="/bob">@bob</a>"#), "{html}");
    assert!(html.contains("@nobody") && html.contains("#99") && !html.contains("issues/99"), "{html}");
    assert_eq!(second.json["author_association"], "none");

    let comments = format!("{issues}/1/comments");
    assert_eq!(send(&app, POST, &comments, None, Some(json!({"body": "x"}))).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(send(&app, POST, &comments, Some(&bob), Some(json!({"body": "  "}))).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    let c = send(&app, POST, &comments, Some(&bob), Some(json!({"body": "First comment, see #2 and #2 again"}))).await;
    assert_eq!(c.status, StatusCode::CREATED, "{}", c.json);
    let id = c.json["id"].as_str().unwrap().to_string();
    assert_eq!(send(&app, GET, &format!("{issues}/1"), None, None).await.json["comments"], 1);

    // Editing keeps the old text; only the author or write access may edit or delete.
    let path = format!("{issues}/comments/{id}");
    assert_eq!(send(&app, PATCH, &path, Some(&carol), Some(json!({"body": "hijack"}))).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&app, DELETE, &path, Some(&carol), None).await.status, StatusCode::FORBIDDEN);
    let edited = send(&app, PATCH, &path, Some(&bob), Some(json!({"body": "Edited comment"}))).await;
    assert_eq!(edited.json["edit_count"], 1);
    let history = send(&app, GET, &format!("{path}/edits"), None, None).await;
    assert_eq!(history.json[0]["body"], "First comment, see #2 and #2 again");
    assert_eq!(history.json[0]["editor"]["login"], "bob");
    let owner_edit = send(&app, PATCH, &path, Some(&alice), Some(json!({"body": "Owner edit"}))).await;
    assert_eq!(owner_edit.json["edit_count"], 2);

    // Issue body history and a rename.
    send(&app, PATCH, &format!("{issues}/1"), Some(&alice), Some(json!({"body": "New body", "title": "First, renamed"}))).await;
    let body_history = send(&app, GET, &format!("{issues}/1/edits"), None, None).await;
    assert!(body_history.json[0]["body"].as_str().unwrap().starts_with("Hello"));
    send(&app, PATCH, &format!("{issues}/1"), Some(&alice), Some(json!({"state": "closed", "state_reason": "not_planned"}))).await;
    send(&app, PATCH, &format!("{issues}/1"), Some(&alice), Some(json!({"state": "open"}))).await;

    let timeline = send(&app, GET, &format!("{issues}/1/timeline"), None, None).await;
    let kinds: Vec<String> = timeline
        .json
        .as_array()
        .unwrap()
        .iter()
        .map(|i| if i["type"] == "comment" { "comment".to_string() } else { i["event"].as_str().unwrap().to_string() })
        .collect();
    assert_eq!(kinds, ["cross_referenced", "comment", "renamed", "closed", "reopened"], "{}", timeline.json);
    let items = timeline.json.as_array().unwrap();
    assert_eq!(items[0]["source"]["number"], 2);
    assert_eq!(items[0]["actor"]["login"], "bob");
    assert_eq!(items[2]["rename"]["from"], "First");
    assert_eq!(items[3]["state_reason"], "not_planned");
    // Issue 2 was mentioned by issue 1's comment exactly once.
    let other = send(&app, GET, &format!("{issues}/2/timeline"), None, None).await;
    assert_eq!(other.json.as_array().unwrap().len(), 1, "{}", other.json);

    assert_eq!(send(&app, DELETE, &path, Some(&bob), None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(send(&app, GET, &format!("{issues}/1"), None, None).await.json["comments"], 0);
    assert_eq!(send(&app, GET, &format!("{path}/edits"), None, None).await.status, StatusCode::NOT_FOUND);

    // Preview renders like a post and respects repository visibility.
    let preview = send(&app, POST, "/api/markdown", None, Some(json!({"text": "see #2 <img src=x onerror=alert(1)>", "repo": "alice/app"}))).await;
    let html = preview.json["html"].as_str().unwrap();
    assert!(html.contains("issues/2") && !html.contains("onerror"), "{html}");
    send(&app, PATCH, repo, Some(&alice), Some(json!({"private": true}))).await;
    let hidden = send(&app, POST, "/api/markdown", Some(&carol), Some(json!({"text": "#2", "repo": "alice/app"}))).await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);

    // Suggestions: a private repository only offers people who can see it.
    assert_eq!(send(&app, GET, &format!("{repo}/mentionables"), Some(&carol), None).await.status, StatusCode::NOT_FOUND);
    send(&app, PUT, &format!("{repo}/collaborators/bob"), Some(&alice), Some(json!({"role": "read"}))).await;
    let names = |v: &serde_json::Value| -> Vec<String> {
        v.as_array().unwrap().iter().map(|u| u["login"].as_str().unwrap().to_string()).collect()
    };
    assert_eq!(names(&send(&app, GET, &format!("{repo}/mentionables"), Some(&bob), None).await.json), ["alice", "bob"]);
    assert_eq!(names(&send(&app, GET, &format!("{repo}/mentionables?q=B"), Some(&bob), None).await.json), ["bob"]);
    send(&app, PATCH, repo, Some(&alice), Some(json!({"private": false}))).await;
    assert_eq!(names(&send(&app, GET, &format!("{repo}/mentionables?q=c"), None, None).await.json), ["carol"]);
    assert_eq!(names(&send(&app, GET, &format!("{repo}/mentionables?q=.%2A"), None, None).await.json), [] as [String; 0]);

    drop_db(&st).await;
}

#[actix_web::test]
async fn access_tokens() {
    let st = state().await;
    let app = app!(st);
    let alice = signup(&app, "alice").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "app", "private": true}))).await;

    let with_token = |method: Method, path: &'static str, token: String, body: Option<serde_json::Value>| {
        let app = &app;
        async move {
            let mut req = actix_web::test::TestRequest::default().method(method).uri(path).insert_header(("authorization", format!("Bearer {token}")));
            if let Some(b) = body {
                req = req.set_json(b);
            }
            let res = actix_web::test::call_service(app, req.to_request()).await;
            let status = res.status();
            let bytes = actix_web::test::read_body(res).await;
            (status, serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or(serde_json::Value::Null))
        }
    };

    // The CLI trades a password for a token; a wrong password gets nothing.
    let bad = send(&app, POST, "/api/auth/token", None, Some(json!({"login": "alice", "password": "nope", "name": "cli"}))).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
    let got = send(&app, POST, "/api/auth/token", None, Some(json!({"login": "alice", "password": "correct horse", "name": "it CLI on laptop"}))).await;
    assert_eq!(got.status, StatusCode::CREATED, "{}", got.json);
    assert!(got.cookie.is_none(), "no browser session comes with it");
    let token = got.json["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("itp_") && token.len() == 44);

    // It acts as the account, including on private repositories.
    let (status, me) = with_token(GET, "/api/user", token.clone(), None).await;
    assert_eq!((status, me["login"].as_str()), (StatusCode::OK, Some("alice")));
    let (status, issue) = with_token(POST, "/api/repos/alice/app/issues", token.clone(), Some(json!({"title": "From the CLI"}))).await;
    assert_eq!((status, issue["author"]["login"].as_str()), (StatusCode::CREATED, Some("alice")));
    // A wrong token is refused outright rather than treated as a visitor.
    assert_eq!(with_token(GET, "/api/health", "itp_wrong".into(), None).await.0, StatusCode::OK, "no auth needed there");
    assert_eq!(with_token(GET, "/api/user/repos", "itp_wrong".into(), None).await.0, StatusCode::UNAUTHORIZED);

    // Tokens are managed from the browser only, and the secret is never listed.
    assert_eq!(with_token(GET, "/api/user/tokens", token.clone(), None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(with_token(POST, "/api/user/tokens", token.clone(), Some(json!({"name": "more"}))).await.0, StatusCode::FORBIDDEN);
    let made = send(&app, POST, "/api/user/tokens", Some(&alice), Some(json!({"name": "CI"}))).await;
    assert_eq!(made.status, StatusCode::CREATED);
    let second = made.json["token"].as_str().unwrap().to_string();
    assert_eq!(send(&app, POST, "/api/user/tokens", Some(&alice), Some(json!({"name": " "}))).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    let list = send(&app, GET, "/api/user/tokens", Some(&alice), None).await;
    assert_eq!(list.json.as_array().unwrap().len(), 2);
    assert!(!list.json.to_string().contains(&token) && !list.json.to_string().contains(&second));
    assert_eq!(list.json[1]["prefix"], token[..8]);
    assert!(list.json[1]["last_used_at"].is_string(), "{}", list.json);
    let stored = st.db.tokens.find_one(mongodb::bson::doc! {}).await.unwrap().unwrap();
    assert!(!stored.hash.contains("itp_"), "only a hash is stored");

    // Revoking from settings, and a token revoking itself.
    let bob = signup(&app, "bob").await;
    let id = made.json["id"].as_str().unwrap();
    assert_eq!(send(&app, DELETE, &format!("/api/user/tokens/{id}"), Some(&bob), None).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&app, DELETE, &format!("/api/user/tokens/{id}"), Some(&alice), None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(with_token(GET, "/api/user", second, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(with_token(DELETE, "/api/user/tokens/current", token.clone(), None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(with_token(GET, "/api/user", token, None).await.0, StatusCode::UNAUTHORIZED);

    drop_db(&st).await;
}

#[actix_web::test]
async fn repeated_bad_passwords_are_slowed_down() {
    let st = state().await;
    let app = app!(st);
    signup(&app, "alice").await;
    let attempt = |password: &'static str| {
        let app = &app;
        async move { send(app, POST, "/api/auth/login", None, Some(json!({"login": "alice", "password": password}))).await }
    };
    for _ in 0..10 {
        assert_eq!(attempt("wrong").await.status, StatusCode::BAD_REQUEST);
    }
    // Now even the right password has to wait, and the CLI endpoint is covered too.
    let blocked = attempt("correct horse").await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(blocked.json["message"].as_str().unwrap().contains("Try again in"), "{}", blocked.json);
    let cli = send(&app, POST, "/api/auth/token", None, Some(json!({"login": "alice", "password": "correct horse", "name": "x"}))).await;
    assert_eq!(cli.status, StatusCode::TOO_MANY_REQUESTS);
    drop_db(&st).await;
}
