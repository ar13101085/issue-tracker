mod common;

use actix_web::http::{Method, StatusCode};
use common::{drop_db, send, signup, state, state_with_mail};
use serde_json::{json, Value};

const GET: Method = Method::GET;
const POST: Method = Method::POST;
const PATCH: Method = Method::PATCH;
const PUT: Method = Method::PUT;
const DELETE: Method = Method::DELETE;

fn numbers(v: &Value) -> Vec<i64> {
    v["items"].as_array().unwrap().iter().map(|i| i["number"].as_i64().unwrap()).collect()
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap().to_string()).collect()
}

fn kinds(timeline: &Value) -> Vec<String> {
    timeline
        .as_array()
        .unwrap()
        .iter()
        .map(|i| if i["type"] == "comment" { "comment".to_string() } else { i["event"].as_str().unwrap().to_string() })
        .collect()
}

/// Percent-encodes a search for the query string.
fn q(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

#[actix_web::test]
async fn labels_milestones_assignees_and_search() {
    let st = state().await;
    let app = app!(st);
    let alice = signup(&app, "alice").await;
    let bob = signup(&app, "bob").await;
    let carol = signup(&app, "carol").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "app"}))).await;
    let repo = "/api/repos/alice/app";
    send(&app, PUT, &format!("{repo}/collaborators/bob"), Some(&alice), Some(json!({"role": "triage"}))).await;

    // A new repository starts with GitHub's default labels.
    let labels = send(&app, GET, &format!("{repo}/labels"), None, None).await;
    assert_eq!(labels.json.as_array().unwrap().len(), 9);
    assert_eq!(labels.json[0]["name"], "bug");
    assert_eq!(labels.json[0]["color"], "d73a4a");

    // Label management needs write access and valid input.
    let new = json!({"name": "Needs Triage", "color": "#FBCA04", "description": "Not looked at yet"});
    assert_eq!(send(&app, POST, &format!("{repo}/labels"), Some(&bob), Some(new.clone())).await.status, StatusCode::FORBIDDEN);
    let made = send(&app, POST, &format!("{repo}/labels"), Some(&alice), Some(new)).await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.json);
    assert_eq!(made.json["color"], "fbca04");
    for bad in [json!({"name": "needs triage", "color": "000000"}), json!({"name": "x", "color": "red"}), json!({"name": " ", "color": "000000"})] {
        assert_eq!(send(&app, POST, &format!("{repo}/labels"), Some(&alice), Some(bad)).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    let m = json!({"title": "v1.0", "description": "First release", "due_on": "2027-01-31T00:00:00Z"});
    let milestone = send(&app, POST, &format!("{repo}/milestones"), Some(&alice), Some(m)).await;
    assert_eq!(milestone.status, StatusCode::CREATED, "{}", milestone.json);
    assert_eq!(milestone.json["number"], 1);
    send(&app, POST, &format!("{repo}/milestones"), Some(&alice), Some(json!({"title": "v2.0"}))).await;
    let dup = send(&app, POST, &format!("{repo}/milestones"), Some(&alice), Some(json!({"title": "V1.0"}))).await;
    assert_eq!(dup.status, StatusCode::UNPROCESSABLE_ENTITY);

    let issues = format!("{repo}/issues");
    // 1: fully organised by the owner.
    let full = json!({"title": "Crash on start", "labels": ["bug", "Needs Triage"], "assignees": ["bob"], "milestone": 1, "issue_type": "bug"});
    let one = send(&app, POST, &issues, Some(&alice), Some(full)).await;
    assert_eq!(one.status, StatusCode::CREATED, "{}", one.json);
    assert_eq!(names(&one.json["labels"]), ["bug", "Needs Triage"]);
    assert_eq!(one.json["assignees"][0]["login"], "bob");
    assert_eq!(one.json["milestone"]["title"], "v1.0");
    assert_eq!(one.json["issue_type"], "Bug");
    // 2: a stranger's organising fields are dropped, not rejected.
    let stranger = send(&app, POST, &issues, Some(&carol), Some(json!({"title": "Docs typo", "labels": ["bug"], "assignees": ["alice"]}))).await;
    assert_eq!(stranger.status, StatusCode::CREATED);
    assert_eq!(stranger.json["labels"].as_array().unwrap().len(), 0);
    assert_eq!(stranger.json["viewer_can_triage"], false);
    // 3 and 4.
    send(&app, POST, &issues, Some(&bob), Some(json!({"title": "Add search", "labels": ["enhancement"], "milestone": 2, "issue_type": "Feature"}))).await;
    send(&app, POST, &issues, Some(&alice), Some(json!({"title": "Search is slow", "body": "takes 10s", "labels": ["bug", "help wanted"]}))).await;

    // Bad organising input is a 422 that names the field.
    for (body, field) in [
        (json!({"labels": ["nope"]}), "labels"),
        (json!({"assignees": ["carol"]}), "assignees"),
        (json!({"milestone": 99}), "milestone"),
        (json!({"issue_type": "Epic"}), "issue_type"),
    ] {
        let r = send(&app, PATCH, &format!("{issues}/2"), Some(&alice), Some(body)).await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{field}");
        assert_eq!(r.json["errors"][0]["field"], field);
    }
    // The author without triage cannot organise their own issue.
    let own = send(&app, PATCH, &format!("{issues}/2"), Some(&carol), Some(json!({"labels": ["bug"]}))).await;
    assert_eq!(own.status, StatusCode::FORBIDDEN);

    // Replacing sets writes one timeline entry per difference.
    let change = json!({"labels": ["bug", "question"], "assignees": ["alice"], "milestone": null, "issue_type": null});
    let changed = send(&app, PATCH, &format!("{issues}/1"), Some(&bob), Some(change)).await;
    assert_eq!(changed.status, StatusCode::OK, "{}", changed.json);
    assert_eq!(names(&changed.json["labels"]), ["bug", "question"]);
    assert!(changed.json["milestone"].is_null() && changed.json["issue_type"].is_null());
    let timeline = send(&app, GET, &format!("{issues}/1/timeline"), None, None).await;
    let mut seen = kinds(&timeline.json);
    seen.sort();
    assert_eq!(seen, ["assigned", "demilestoned", "labeled", "type_changed", "unassigned", "unlabeled"], "{}", timeline.json);
    // Sending the same sets again changes nothing.
    send(&app, PATCH, &format!("{issues}/1"), Some(&bob), Some(json!({"labels": ["question", "bug"], "assignees": ["alice"]}))).await;
    assert_eq!(send(&app, GET, &format!("{issues}/1/timeline"), None, None).await.json.as_array().unwrap().len(), 6);

    send(&app, PATCH, &format!("{issues}/3"), Some(&alice), Some(json!({"state": "closed"}))).await;

    let search = |text: &'static str, who: Option<&'static str>| {
        let path = format!("{issues}?q={}", q(text));
        let app = &app;
        async move { numbers(&send(app, GET, &path, who, None).await.json) }
    };
    let alice_s: &'static str = Box::leak(alice.clone().into_boxed_str());
    assert_eq!(search("is:issue state:open", None).await, [4, 2, 1]);
    assert_eq!(search("is:issue", None).await, [4, 3, 2, 1], "no state means all");
    assert_eq!(search("label:bug", None).await, [4, 1]);
    assert_eq!(search("label:bug label:\"help wanted\"", None).await, [4]);
    assert_eq!(search("label:BUG -label:question", None).await, [4]);
    assert_eq!(search("label:nope", None).await, [] as [i64; 0]);
    assert_eq!(search("no:label", None).await, [2]);
    assert_eq!(search("no:assignee state:open", None).await, [4, 2]);
    assert_eq!(search("assignee:alice", None).await, [1]);
    assert_eq!(search("assignee:@me", Some(alice_s)).await, [1]);
    assert_eq!(search("assignee:@me", None).await, [] as [i64; 0]);
    assert_eq!(search("author:carol", None).await, [2]);
    assert_eq!(search("author:@me is:closed", Some(alice_s)).await, [] as [i64; 0]);
    assert_eq!(search("milestone:\"v2.0\"", None).await, [3]);
    assert_eq!(search("no:milestone", None).await, [4, 2, 1]);
    assert_eq!(search("type:feature", None).await, [3]);
    assert_eq!(search("no:type sort:created-asc", None).await, [1, 2, 4]);
    assert_eq!(search("search", None).await, [4, 3]);
    assert_eq!(search("search in:body", None).await, [] as [i64; 0]);
    assert_eq!(search("10s in:body", None).await, [4]);
    assert_eq!(search("is:open label:bug slow", None).await, [4]);
    // Counts follow the filters but not the state.
    let counted = send(&app, GET, &format!("{issues}?q={}", q("is:open search")), None, None).await;
    assert_eq!((counted.json["open_count"].as_u64(), counted.json["closed_count"].as_u64()), (Some(1), Some(1)));

    // Label counts, rename, delete.
    let labels = send(&app, GET, &format!("{repo}/labels"), None, None).await;
    let bug = labels.json.as_array().unwrap().iter().find(|l| l["name"] == "bug").unwrap();
    assert_eq!(bug["open_issues"], 2);
    let renamed = send(&app, PATCH, &format!("{repo}/labels/bug"), Some(&alice), Some(json!({"new_name": "defect", "color": "ff0000"}))).await;
    assert_eq!(renamed.json["name"], "defect");
    assert_eq!(search("label:defect", None).await, [4, 1]);
    assert_eq!(send(&app, DELETE, &format!("{repo}/labels/help%20wanted"), Some(&alice), None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(names(&send(&app, GET, &format!("{issues}/4"), None, None).await.json["labels"]), ["defect"]);

    // Milestone progress, closing and deleting.
    send(&app, PATCH, &format!("{issues}/4"), Some(&alice), Some(json!({"milestone": 2}))).await;
    let v2 = send(&app, GET, &format!("{repo}/milestones/2"), None, None).await;
    assert_eq!((v2.json["open_issues"].as_u64(), v2.json["closed_issues"].as_u64()), (Some(1), Some(1)));
    let closed = send(&app, PATCH, &format!("{repo}/milestones/2"), Some(&alice), Some(json!({"state": "closed", "due_on": null}))).await;
    assert_eq!(closed.json["state"], "closed");
    assert!(closed.json["closed_at"].is_string());
    assert_eq!(names_of_titles(&send(&app, GET, &format!("{repo}/milestones"), None, None).await.json), ["v1.0"]);
    assert_eq!(names_of_titles(&send(&app, GET, &format!("{repo}/milestones?state=closed"), None, None).await.json), ["v2.0"]);
    assert_eq!(send(&app, DELETE, &format!("{repo}/milestones/2"), Some(&bob), None).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&app, DELETE, &format!("{repo}/milestones/2"), Some(&alice), None).await.status, StatusCode::NO_CONTENT);
    assert!(send(&app, GET, &format!("{issues}/4"), None, None).await.json["milestone"].is_null());

    let assignable = send(&app, GET, &format!("{repo}/assignees"), None, None).await;
    let logins: Vec<&str> = assignable.json.as_array().unwrap().iter().map(|u| u["login"].as_str().unwrap()).collect();
    assert_eq!(logins, ["alice", "bob"]);

    drop_db(&st).await;
}

fn names_of_titles(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|m| m["title"].as_str().unwrap().to_string()).collect()
}

#[actix_web::test]
async fn lock_pin_sub_issues_bulk_and_reactions() {
    let st = state().await;
    let app = app!(st);
    let alice = signup(&app, "alice").await;
    let bob = signup(&app, "bob").await;
    let carol = signup(&app, "carol").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "app"}))).await;
    let repo = "/api/repos/alice/app";
    let issues = format!("{repo}/issues");
    send(&app, PUT, &format!("{repo}/collaborators/bob"), Some(&alice), Some(json!({"role": "triage"}))).await;
    for n in 1..=5 {
        send(&app, POST, &issues, Some(&carol), Some(json!({"title": format!("Issue {n}")}))).await;
    }

    // Locking: only triage and up; afterwards strangers cannot comment or react.
    assert_eq!(send(&app, PUT, &format!("{issues}/1/lock"), Some(&carol), None).await.status, StatusCode::FORBIDDEN);
    let bad = send(&app, PUT, &format!("{issues}/1/lock"), Some(&bob), Some(json!({"lock_reason": "bored"}))).await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    let locked = send(&app, PUT, &format!("{issues}/1/lock"), Some(&bob), Some(json!({"lock_reason": "too heated"}))).await;
    assert_eq!(locked.json["locked"], true);
    assert_eq!(locked.json["lock_reason"], "too heated");
    let as_author = send(&app, GET, &format!("{issues}/1"), Some(&carol), None).await;
    assert_eq!(as_author.json["viewer_can_comment"], false);
    assert_eq!(send(&app, POST, &format!("{issues}/1/comments"), Some(&carol), Some(json!({"body": "let me in"}))).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&app, PUT, &format!("{issues}/1/reactions/+1"), Some(&carol), None).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&app, POST, &format!("{issues}/1/comments"), Some(&bob), Some(json!({"body": "team only"}))).await.status, StatusCode::CREATED);
    let unlocked = send(&app, DELETE, &format!("{issues}/1/lock"), Some(&bob), None).await;
    assert_eq!(unlocked.json["locked"], false);
    assert_eq!(send(&app, POST, &format!("{issues}/1/comments"), Some(&carol), Some(json!({"body": "thanks"}))).await.status, StatusCode::CREATED);

    // Pinning: write access, three at most.
    assert_eq!(send(&app, PUT, &format!("{issues}/1/pin"), Some(&bob), None).await.status, StatusCode::FORBIDDEN);
    for n in 1..=3 {
        assert_eq!(send(&app, PUT, &format!("{issues}/{n}/pin"), Some(&alice), None).await.json["pinned"], true);
    }
    assert_eq!(send(&app, PUT, &format!("{issues}/4/pin"), Some(&alice), None).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    let pinned = send(&app, GET, &format!("{issues}/pinned"), None, None).await;
    assert_eq!(pinned.json.as_array().unwrap().len(), 3);
    send(&app, DELETE, &format!("{issues}/2/pin"), Some(&alice), None).await;
    assert_eq!(send(&app, GET, &format!("{issues}/pinned"), None, None).await.json.as_array().unwrap().len(), 2);

    // Sub-issues: one parent each, no loops.
    let sub = |parent: i64, child: i64, who: &'static str| {
        let path = format!("{issues}/{parent}/sub_issues");
        let app = &app;
        async move { send(app, POST, &path, Some(who), Some(json!({"number": child}))).await }
    };
    let bob_s: &'static str = Box::leak(bob.clone().into_boxed_str());
    let carol_s: &'static str = Box::leak(carol.clone().into_boxed_str());
    assert_eq!(sub(1, 2, carol_s).await.status, StatusCode::FORBIDDEN);
    let parent = sub(1, 2, bob_s).await;
    assert_eq!(parent.status, StatusCode::OK, "{}", parent.json);
    assert_eq!(parent.json["sub_issues"][0]["number"], 2);
    assert_eq!(sub(2, 3, bob_s).await.status, StatusCode::OK);
    assert_eq!(sub(1, 1, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "itself");
    assert_eq!(sub(4, 2, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "second parent");
    assert_eq!(sub(3, 1, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "loop");
    assert_eq!(sub(1, 99, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "missing");
    let child = send(&app, GET, &format!("{issues}/2"), None, None).await;
    assert_eq!(child.json["parent"]["number"], 1);
    assert_eq!(child.json["sub_issues"][0]["number"], 3);
    let removed = send(&app, DELETE, &format!("{issues}/1/sub_issues/2"), Some(&bob), None).await;
    assert_eq!(removed.json["sub_issues"].as_array().unwrap().len(), 0);
    assert_eq!(send(&app, DELETE, &format!("{issues}/1/sub_issues/2"), Some(&bob), None).await.status, StatusCode::NOT_FOUND);

    // Blocking: #3 waits on #4, #4 waits on #1; nothing may wait on itself, twice, or in a circle.
    let block = |issue: i64, by: i64, who: &'static str| {
        let path = format!("{issues}/{issue}/dependencies/blocked_by");
        let app = &app;
        async move { send(app, POST, &path, Some(who), Some(json!({"number": by}))).await }
    };
    assert_eq!(block(3, 4, carol_s).await.status, StatusCode::FORBIDDEN);
    let blocked = block(3, 4, bob_s).await;
    assert_eq!(blocked.status, StatusCode::OK, "{}", blocked.json);
    assert_eq!(blocked.json["blocked_by"][0]["number"], 4);
    assert_eq!(block(4, 1, bob_s).await.status, StatusCode::OK);
    assert_eq!(block(3, 3, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "itself");
    assert_eq!(block(3, 4, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "twice");
    assert_eq!(block(4, 3, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "each other");
    assert_eq!(block(1, 3, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "circle");
    assert_eq!(block(3, 99, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "missing");
    let middle = send(&app, GET, &format!("{issues}/4"), None, None).await;
    assert_eq!(middle.json["blocked_by"][0]["number"], 1);
    assert_eq!(middle.json["blocking"][0]["number"], 3);
    assert_eq!(kinds(&send(&app, GET, &format!("{issues}/4/timeline"), None, None).await.json), ["blocking_added", "blocked_by_added"]);
    let unblocked = send(&app, DELETE, &format!("{issues}/3/dependencies/blocked_by/4"), Some(&bob), None).await;
    assert_eq!(unblocked.json["blocked_by"].as_array().unwrap().len(), 0);
    assert_eq!(send(&app, DELETE, &format!("{issues}/3/dependencies/blocked_by/4"), Some(&bob), None).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&app, GET, &format!("{issues}/4"), None, None).await.json["blocking"].as_array().unwrap().len(), 0);

    // Related: one link, visible from both issues.
    let relate = |issue: i64, other: i64, who: &'static str| {
        let path = format!("{issues}/{issue}/relations");
        let app = &app;
        async move { send(app, POST, &path, Some(who), Some(json!({"number": other}))).await }
    };
    assert_eq!(relate(3, 1, carol_s).await.status, StatusCode::FORBIDDEN);
    let related = relate(3, 1, bob_s).await;
    assert_eq!(related.status, StatusCode::OK, "{}", related.json);
    assert_eq!(related.json["related"][0]["number"], 1);
    assert_eq!(send(&app, GET, &format!("{issues}/1"), None, None).await.json["related"][0]["number"], 3);
    assert_eq!(relate(1, 3, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "twice, from the other side");
    assert_eq!(relate(3, 3, bob_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "itself");
    let unrelated = send(&app, DELETE, &format!("{issues}/1/relations/3"), Some(&bob), None).await;
    assert_eq!(unrelated.json["related"].as_array().unwrap().len(), 0);
    assert_eq!(send(&app, GET, &format!("{issues}/3"), None, None).await.json["related"].as_array().unwrap().len(), 0);
    assert_eq!(send(&app, DELETE, &format!("{issues}/3/relations/1"), Some(&bob), None).await.status, StatusCode::NOT_FOUND);
    assert_eq!(kinds(&send(&app, GET, &format!("{issues}/2/timeline"), None, None).await.json), ["pinned", "unpinned", "parent_added", "sub_issue_added", "parent_removed"]);

    // Bulk edits behave like the single edit, once per issue.
    let bulk = json!({"numbers": [4, 5], "state": "closed", "state_reason": "not_planned", "add_labels": ["wontfix", "bug"], "add_assignees": ["bob"]});
    assert_eq!(send(&app, POST, &format!("{issues}/bulk"), Some(&carol), Some(bulk.clone())).await.status, StatusCode::FORBIDDEN);
    let done = send(&app, POST, &format!("{issues}/bulk"), Some(&bob), Some(bulk)).await;
    assert_eq!(done.json["updated"], 2, "{}", done.json);
    let five = send(&app, GET, &format!("{issues}/5"), None, None).await;
    assert_eq!((five.json["state"].as_str(), five.json["state_reason"].as_str()), (Some("closed"), Some("not_planned")));
    assert_eq!(names(&five.json["labels"]), ["bug", "wontfix"]);
    assert_eq!(five.json["assignees"][0]["login"], "bob");
    send(&app, POST, &format!("{issues}/bulk"), Some(&bob), Some(json!({"numbers": [5], "remove_labels": ["bug"], "remove_assignees": ["bob"], "state": "open"}))).await;
    let five = send(&app, GET, &format!("{issues}/5"), None, None).await;
    assert_eq!(names(&five.json["labels"]), ["wontfix"]);
    assert_eq!(five.json["assignees"].as_array().unwrap().len(), 0);
    assert_eq!(five.json["state"], "open");
    assert_eq!(send(&app, POST, &format!("{issues}/bulk"), Some(&bob), Some(json!({"numbers": []}))).await.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Reactions: one per user and emoji, on issues and comments.
    assert_eq!(send(&app, PUT, &format!("{issues}/3/reactions/heart"), None, None).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(send(&app, PUT, &format!("{issues}/3/reactions/poop"), Some(&bob), None).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    send(&app, PUT, &format!("{issues}/3/reactions/heart"), Some(&bob), None).await;
    send(&app, PUT, &format!("{issues}/3/reactions/heart"), Some(&bob), None).await;
    let groups = send(&app, PUT, &format!("{issues}/3/reactions/heart"), Some(&carol), None).await;
    assert_eq!(groups.json[0]["count"], 2, "{}", groups.json);
    assert_eq!(groups.json[0]["viewer_reacted"], true);
    let seen = send(&app, GET, &format!("{issues}/3"), Some(&alice), None).await;
    assert_eq!(seen.json["reactions"][0]["content"], "heart");
    assert_eq!(seen.json["reactions"][0]["viewer_reacted"], false);
    assert_eq!(send(&app, DELETE, &format!("{issues}/3/reactions/heart"), Some(&bob), None).await.json[0]["count"], 1);
    let c = send(&app, POST, &format!("{issues}/3/comments"), Some(&bob), Some(json!({"body": "nice"}))).await;
    let id = c.json["id"].as_str().unwrap();
    send(&app, PUT, &format!("{issues}/comments/{id}/reactions/rocket"), Some(&alice), None).await;
    let timeline = send(&app, GET, &format!("{issues}/3/timeline"), Some(&alice), None).await;
    let comment = timeline.json.as_array().unwrap().iter().find(|i| i["type"] == "comment").unwrap();
    assert_eq!(comment["reactions"][0]["content"], "rocket");
    assert_eq!(comment["reactions"][0]["users"][0], "alice");

    drop_db(&st).await;
}

#[actix_web::test]
async fn transfer_and_templates() {
    let st = state().await;
    let app = app!(st);
    // The first account is the site admin, who may go anywhere; keep it out of the way.
    let _root = signup(&app, "root").await;
    let alice = signup(&app, "alice").await;
    let bob = signup(&app, "bob").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "old"}))).await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "new"}))).await;
    send(&app, POST, "/api/user/repos", Some(&bob), Some(json!({"name": "bobs", "private": true}))).await;
    let old = "/api/repos/alice/old";
    send(&app, POST, &format!("{old}/labels"), Some(&alice), Some(json!({"name": "only-here", "color": "000000"}))).await;
    send(&app, POST, &format!("{old}/milestones"), Some(&alice), Some(json!({"title": "m"}))).await;
    send(&app, POST, &format!("{old}/issues"), Some(&alice), Some(json!({"title": "Moves", "labels": ["bug", "only-here"], "milestone": 1}))).await;
    send(&app, POST, &format!("{old}/issues/1/comments"), Some(&bob), Some(json!({"body": "comes along"}))).await;
    for _ in 0..2 {
        send(&app, POST, "/api/repos/alice/new/issues", Some(&alice), Some(json!({"title": "filler"}))).await;
    }

    let go = |repo: &'static str, who: &'static str| {
        let app = &app;
        async move { send(app, POST, "/api/repos/alice/old/issues/1/transfer", Some(who), Some(json!({"repo": repo}))).await }
    };
    let alice_s: &'static str = Box::leak(alice.clone().into_boxed_str());
    let bob_s: &'static str = Box::leak(bob.clone().into_boxed_str());
    assert_eq!(go("alice/new", bob_s).await.status, StatusCode::FORBIDDEN);
    for bad in ["nonsense", "alice/old", "alice/missing", "bob/bobs"] {
        assert_eq!(go(bad, alice_s).await.status, StatusCode::UNPROCESSABLE_ENTITY, "{bad}");
    }
    let moved = go("alice/new", alice_s).await;
    assert_eq!(moved.status, StatusCode::OK, "{}", moved.json);
    assert_eq!((moved.json["repo"].as_str(), moved.json["number"].as_i64()), (Some("new"), Some(3)));
    assert_eq!(send(&app, GET, &format!("{old}/issues/1"), None, None).await.status, StatusCode::NOT_FOUND);
    let there = send(&app, GET, "/api/repos/alice/new/issues/3", None, None).await;
    assert_eq!(there.json["title"], "Moves");
    assert_eq!(there.json["comments"], 1);
    assert_eq!(names(&there.json["labels"]), ["bug"], "only labels the destination has");
    assert!(there.json["milestone"].is_null());
    let timeline = send(&app, GET, "/api/repos/alice/new/issues/3/timeline", None, None).await;
    assert_eq!(kinds(&timeline.json), ["comment", "transferred"]);
    assert_eq!(timeline.json[1]["detail"], "alice/old");

    // Templates: admin only, validated, returned to everyone who can see the repository.
    let list = json!([
        {"name": "Bug report", "about": "Something is broken", "title": "[Bug]: ", "body": "## Steps\n", "labels": ["bug"], "issue_type": "bug"},
        {"name": "Feature request", "body": "## Idea\n"}
    ]);
    assert_eq!(send(&app, PUT, &format!("{old}/issue-templates"), Some(&bob), Some(list.clone())).await.status, StatusCode::FORBIDDEN);
    let saved = send(&app, PUT, &format!("{old}/issue-templates"), Some(&alice), Some(list)).await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.json);
    assert_eq!(saved.json[0]["issue_type"], "Bug");
    assert_eq!(send(&app, GET, &format!("{old}/issue-templates"), None, None).await.json.as_array().unwrap().len(), 2);
    for bad in [json!([{"name": ""}]), json!([{"name": "a"}, {"name": "A"}]), json!([{"name": "a", "labels": ["nope"]}]), json!([{"name": "a", "issue_type": "Epic"}])] {
        assert_eq!(send(&app, PUT, &format!("{old}/issue-templates"), Some(&alice), Some(bad)).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    drop_db(&st).await;
}

#[actix_web::test]
async fn notifications_subscriptions_and_email() {
    let (st, mut mail) = state_with_mail().await;
    let app = app!(st);
    let alice = signup(&app, "alice").await;
    let bob = signup(&app, "bob").await;
    let carol = signup(&app, "carol").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "app"}))).await;
    let issues = "/api/repos/alice/app/issues";
    let mut hub = st.hub.subscribe();

    let inbox = |who: &'static str, filter: &'static str| {
        let app = &app;
        async move { send(app, GET, &format!("/api/notifications?filter={filter}"), Some(who), None).await.json }
    };
    let alice_s: &'static str = Box::leak(alice.clone().into_boxed_str());
    let bob_s: &'static str = Box::leak(bob.clone().into_boxed_str());
    let carol_s: &'static str = Box::leak(carol.clone().into_boxed_str());
    let mails = |rx: &mut tokio::sync::mpsc::UnboundedReceiver<issue_tracker::mail::Mail>| {
        let mut to = Vec::new();
        while let Ok(m) = rx.try_recv() {
            to.push(m.to);
        }
        to.sort();
        to
    };

    assert_eq!(send(&app, GET, "/api/notifications", None, None).await.status, StatusCode::UNAUTHORIZED);

    // Bob opens an issue mentioning carol: the owner and carol hear about it, bob does not.
    send(&app, POST, issues, Some(&bob), Some(json!({"title": "Help", "body": "ping @carol"}))).await;
    assert_eq!(inbox(bob_s, "inbox").await["total_count"], 0);
    let c = inbox(carol_s, "inbox").await;
    assert_eq!((c["unread_count"].as_u64(), c["items"][0]["reason"].as_str()), (Some(1), Some("mention")), "{c}");
    assert_eq!(c["items"][0]["repo"], "alice/app");
    assert_eq!(c["items"][0]["issue"]["title"], "Help");
    assert_eq!(c["items"][0]["last_actor"]["login"], "bob");
    assert_eq!(inbox(alice_s, "inbox").await["items"][0]["reason"], "subscribed");
    assert_eq!(mails(&mut mail), ["alice@example.test", "carol@example.test"]);

    // The stream announced the repository change and each inbox change.
    let mut repo_signals = 0;
    let mut user_signals = 0;
    while let Ok(s) = hub.try_recv() {
        if s.repo_id.is_some() {
            repo_signals += 1;
        } else if s.user_id.is_some() {
            user_signals += 1;
        }
    }
    assert_eq!((repo_signals, user_signals), (1, 2));

    // Opening the issue reads the notification.
    send(&app, GET, &format!("{issues}/1"), Some(&carol), None).await;
    let c = inbox(carol_s, "inbox").await;
    assert_eq!((c["unread_count"].as_u64(), c["items"][0]["unread"].as_bool()), (Some(0), Some(false)));
    assert_eq!(inbox(carol_s, "unread").await["total_count"], 0);

    // A comment reaches everyone following the issue except its author.
    send(&app, POST, &format!("{issues}/1/comments"), Some(&alice), Some(json!({"body": "on it"}))).await;
    assert_eq!(inbox(bob_s, "inbox").await["items"][0]["reason"], "author");
    assert_eq!(inbox(carol_s, "unread").await["total_count"], 1);
    assert_eq!(inbox(alice_s, "unread").await["total_count"], 1, "her own comment adds nothing new");
    assert_eq!(mails(&mut mail), ["bob@example.test", "carol@example.test"]);

    // Unsubscribing stops comments, but a direct mention still gets through.
    let seen = send(&app, GET, &format!("{issues}/1"), Some(&carol), None).await;
    assert_eq!(seen.json["viewer_subscribed"], true);
    send(&app, PUT, &format!("{issues}/1/subscription"), Some(&carol), Some(json!({"subscribed": false}))).await;
    assert_eq!(send(&app, GET, &format!("{issues}/1"), Some(&carol), None).await.json["viewer_subscribed"], false);
    send(&app, POST, &format!("{issues}/1/comments"), Some(&alice), Some(json!({"body": "more"}))).await;
    assert_eq!(inbox(carol_s, "unread").await["total_count"], 0);
    assert_eq!(mails(&mut mail), ["bob@example.test"]);
    send(&app, POST, &format!("{issues}/1/comments"), Some(&alice), Some(json!({"body": "@carol again"}))).await;
    assert_eq!(inbox(carol_s, "unread").await["total_count"], 1);
    mails(&mut mail);

    // Email can be switched off per account; the inbox still fills.
    let off = send(&app, PATCH, "/api/user", Some(&bob), Some(json!({"email_notifications": false, "name": "Bob B"}))).await;
    assert_eq!((off.json["email_notifications"].as_bool(), off.json["name"].as_str()), (Some(false), Some("Bob B")));
    send(&app, PATCH, &format!("{issues}/1"), Some(&alice), Some(json!({"state": "closed"}))).await;
    assert_eq!(mails(&mut mail), [] as [String; 0], "bob opted out, carol unsubscribed");
    assert_eq!(inbox(bob_s, "unread").await["total_count"], 1);

    // Assigning notifies the assignee.
    send(&app, PUT, "/api/repos/alice/app/collaborators/carol", Some(&alice), Some(json!({"role": "read"}))).await;
    send(&app, POST, issues, Some(&alice), Some(json!({"title": "Second"}))).await;
    send(&app, PATCH, &format!("{issues}/2"), Some(&alice), Some(json!({"assignees": ["carol"]}))).await;
    let c = inbox(carol_s, "inbox").await;
    assert_eq!(c["items"][0]["reason"], "assign");
    assert_eq!(c["total_count"], 2);

    // Done, undone, unread and mark all read; never someone else's row.
    let id = c["items"][0]["id"].as_str().unwrap().to_string();
    let row = format!("/api/notifications/{id}");
    assert_eq!(send(&app, PATCH, &row, Some(&bob), Some(json!({"done": true}))).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&app, PATCH, &row, Some(&carol), Some(json!({"done": true}))).await.status, StatusCode::NO_CONTENT);
    assert_eq!(inbox(carol_s, "inbox").await["total_count"], 1);
    assert_eq!(inbox(carol_s, "done").await["total_count"], 1);
    send(&app, PATCH, &row, Some(&carol), Some(json!({"done": false, "unread": true}))).await;
    assert_eq!(inbox(carol_s, "unread").await["total_count"], 2);
    assert_eq!(send(&app, POST, "/api/notifications/read", Some(&carol), None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(inbox(carol_s, "unread").await["total_count"], 0);

    // Nobody is told about a private repository they cannot open.
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "secret", "private": true}))).await;
    mails(&mut mail);
    send(&app, POST, "/api/repos/alice/secret/issues", Some(&alice), Some(json!({"title": "Hidden", "body": "@bob @carol"}))).await;
    assert_eq!(inbox(bob_s, "inbox").await["items"].as_array().unwrap().iter().filter(|n| n["repo"] == "alice/secret").count(), 0);
    assert_eq!(mails(&mut mail), [] as [String; 0]);

    // Deleting an issue clears its notifications.
    send(&app, DELETE, &format!("{issues}/2"), Some(&alice), None).await;
    assert_eq!(inbox(carol_s, "inbox").await["total_count"], 1);

    drop_db(&st).await;
}

#[actix_web::test]
async fn attachments_follow_repository_visibility() {
    let st = state().await;
    let app = app!(st);
    let alice = signup(&app, "alice").await;
    let bob = signup(&app, "bob").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "secret", "private": true}))).await;

    let upload = |name: &'static str, body: Vec<u8>, who: Option<&'static str>| {
        let app = &app;
        async move {
            let mut req = actix_web::test::TestRequest::post().uri(&format!("/api/repos/alice/secret/attachments?name={name}"));
            if let Some(c) = who {
                req = req.insert_header(("cookie", format!("it_session={c}")));
            }
            let res = actix_web::test::call_service(app, req.set_payload(body).to_request()).await;
            let status = res.status();
            let bytes = actix_web::test::read_body(res).await;
            (status, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null))
        }
    };
    let fetch = |url: String, who: Option<&'static str>| {
        let app = &app;
        async move {
            let mut req = actix_web::test::TestRequest::get().uri(&url);
            if let Some(c) = who {
                req = req.insert_header(("cookie", format!("it_session={c}")));
            }
            let res = actix_web::test::call_service(app, req.to_request()).await;
            let header = |n: &str| res.headers().get(n).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
            (res.status(), header("content-type"), header("content-disposition"))
        }
    };
    let alice_s: &'static str = Box::leak(alice.clone().into_boxed_str());
    let bob_s: &'static str = Box::leak(bob.clone().into_boxed_str());
    let png = b"\x89PNG\r\n\x1a\n-not-really-pixels".to_vec();

    assert_eq!(upload("a.png", png.clone(), None).await.0, StatusCode::NOT_FOUND, "anonymous cannot even see the repository");
    assert_eq!(upload("a.png", png.clone(), Some(bob_s)).await.0, StatusCode::NOT_FOUND);
    assert_eq!(upload("a.png", Vec::new(), Some(alice_s)).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    let (status, image) = upload("../My%20Shot.png", png, Some(alice_s)).await;
    assert_eq!(status, StatusCode::CREATED, "{image}");
    assert_eq!(image["content_type"], "image/png");
    assert_eq!(image["name"], "My-Shot.png");
    let url = image["url"].as_str().unwrap().to_string();
    assert_eq!(image["markdown"], format!("![My-Shot.png]({url})"));

    let (status, kind, disposition) = fetch(url.clone(), Some(alice_s)).await;
    assert_eq!((status, kind.as_str(), disposition.as_str()), (StatusCode::OK, "image/png", "inline"));
    assert_eq!(fetch(url.clone(), Some(bob_s)).await.0, StatusCode::NOT_FOUND, "private repository, private files");
    assert_eq!(fetch(url.clone(), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(fetch("/attachments/000000000000000000000000/x.png".into(), Some(alice_s)).await.0, StatusCode::NOT_FOUND);

    // A page disguised as an image is stored as a plain download, never rendered.
    let (_, html) = upload("evil.png", b"<html><script>alert(1)</script>".to_vec(), Some(alice_s)).await;
    assert_eq!(html["content_type"], "application/octet-stream");
    assert!(html["markdown"].as_str().unwrap().starts_with("[evil.png]"));
    let (_, kind, disposition) = fetch(html["url"].as_str().unwrap().to_string(), Some(alice_s)).await;
    assert_eq!(kind, "application/octet-stream");
    assert!(disposition.starts_with("attachment"), "{disposition}");

    // Uploaded images render in comments; the link survives sanitizing.
    let issue = send(&app, POST, "/api/repos/alice/secret/issues", Some(&alice), Some(json!({"title": "Shot", "body": image["markdown"]}))).await;
    assert!(issue.json["body_html"].as_str().unwrap().contains(&format!("<img src=\"{url}\"")), "{}", issue.json["body_html"]);

    // Once public, anyone may fetch it.
    send(&app, PATCH, "/api/repos/alice/secret", Some(&alice), Some(json!({"private": false}))).await;
    assert_eq!(fetch(url, None).await.0, StatusCode::OK);

    drop_db(&st).await;
}

#[actix_web::test]
async fn projects_fields_and_ordering() {
    let st = state().await;
    let app = app!(st);
    let _root = signup(&app, "root").await;
    let alice = signup(&app, "alice").await;
    let bob = signup(&app, "bob").await;
    let carol = signup(&app, "carol").await;
    send(&app, POST, "/api/user/repos", Some(&alice), Some(json!({"name": "app", "private": true}))).await;
    let repo = "/api/repos/alice/app";
    send(&app, PUT, &format!("{repo}/collaborators/bob"), Some(&alice), Some(json!({"role": "triage"}))).await;
    for n in 1..=4 {
        send(&app, POST, &format!("{repo}/issues"), Some(&alice), Some(json!({"title": format!("Issue {n}"), "labels": ["bug"]}))).await;
    }
    let projects = format!("{repo}/projects");

    // Write access creates a project; it starts with the Status field.
    assert_eq!(send(&app, POST, &projects, Some(&bob), Some(json!({"title": "Roadmap"}))).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&app, POST, &projects, Some(&alice), Some(json!({"title": " "}))).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    let made = send(&app, POST, &projects, Some(&alice), Some(json!({"title": "Roadmap", "description": "Q1"}))).await;
    assert_eq!(made.status, StatusCode::CREATED, "{}", made.json);
    assert_eq!(made.json["number"], 1);
    let status_names: Vec<&str> = made.json["fields"][0]["options"].as_array().unwrap().iter().map(|o| o["name"].as_str().unwrap()).collect();
    assert_eq!(status_names, ["Todo", "In Progress", "Done"]);
    let option = |name: &str| made.json["fields"][0]["options"].as_array().unwrap().iter().find(|o| o["name"] == name).unwrap()["id"].as_str().unwrap().to_string();
    let (todo, doing) = (option("Todo"), option("In Progress"));
    let p = format!("{projects}/1");

    // A private repository's projects are invisible to outsiders.
    assert_eq!(send(&app, GET, &p, Some(&carol), None).await.status, StatusCode::NOT_FOUND);
    assert_eq!(send(&app, GET, &projects, None, None).await.status, StatusCode::NOT_FOUND);

    // Triage adds items; each issue once; unknown issues are refused.
    let mut item = std::collections::HashMap::new();
    for n in 1..=4 {
        let r = send(&app, POST, &format!("{p}/items"), Some(&bob), Some(json!({"issue_number": n}))).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.json);
        let row = r.json["items"].as_array().unwrap().iter().find(|i| i["issue"]["number"] == n).unwrap();
        assert_eq!(row["issue"]["labels"][0]["name"], "bug", "items carry the whole issue");
        item.insert(n, row["id"].as_str().unwrap().to_string());
    }
    assert_eq!(send(&app, POST, &format!("{p}/items"), Some(&bob), Some(json!({"issue_number": 1}))).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(send(&app, POST, &format!("{p}/items"), Some(&bob), Some(json!({"issue_number": 99}))).await.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Order inside a status column, as the board shows it.
    let column = |v: &Value, status: Option<&str>| -> Vec<i64> {
        v["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["values"]["status"].as_str() == status)
            .map(|i| i["issue"]["number"].as_i64().unwrap())
            .collect()
    };
    let set = |n: i64, body: Value| {
        let path = format!("{p}/items/{}", item[&n]);
        let app = &app;
        let who = bob.clone();
        async move { send(app, PATCH, &path, Some(&who), Some(body)).await }
    };
    set(1, json!({"values": {"status": todo}})).await;
    set(2, json!({"values": {"status": todo}})).await;
    let board = set(3, json!({"values": {"status": todo}})).await;
    assert_eq!(column(&board.json, Some(&todo)), [1, 2, 3], "a new arrival goes to the end");
    assert_eq!(column(&board.json, None), [4]);
    // Drag 3 in front of 1, then 1 to the end, then 2 between them.
    assert_eq!(column(&set(3, json!({"before": item[&1]})).await.json, Some(&todo)), [3, 1, 2]);
    assert_eq!(column(&set(1, json!({"before": null})).await.json, Some(&todo)), [3, 2, 1]);
    // Drop into another column at a given place.
    set(4, json!({"values": {"status": doing}})).await;
    let moved = set(2, json!({"values": {"status": doing}, "before": item[&4]})).await;
    assert_eq!(column(&moved.json, Some(&doing)), [2, 4]);
    assert_eq!(column(&moved.json, Some(&todo)), [3, 1]);
    let bad = set(1, json!({"before": item[&4]})).await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY, "the target is in another column");
    assert_eq!(set(1, json!({"values": {"status": "nope"}})).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    // Clearing the status returns the card to "no status".
    assert_eq!(column(&set(3, json!({"values": {"status": null}})).await.json, None), [3]);

    // Custom fields: write access to define, each kind validates its values.
    let fields = format!("{p}/fields");
    assert_eq!(send(&app, POST, &fields, Some(&bob), Some(json!({"name": "Estimate", "kind": "number"}))).await.status, StatusCode::FORBIDDEN);
    let with = send(&app, POST, &fields, Some(&alice), Some(json!({"name": "Estimate", "kind": "number"}))).await;
    assert_eq!(with.status, StatusCode::OK, "{}", with.json);
    let field_id = |v: &Value, name: &str| v["fields"].as_array().unwrap().iter().find(|f| f["name"] == name).unwrap()["id"].as_str().unwrap().to_string();
    let estimate = field_id(&with.json, "Estimate");
    let with = send(&app, POST, &fields, Some(&alice), Some(json!({"name": "Due", "kind": "date"}))).await;
    let due = field_id(&with.json, "Due");
    let with = send(&app, POST, &fields, Some(&alice), Some(json!({"name": "Notes", "kind": "text"}))).await;
    let notes = field_id(&with.json, "Notes");
    let with = send(&app, POST, &fields, Some(&alice), Some(json!({"name": "Priority", "kind": "single_select", "options": ["High", "Low"]}))).await;
    let priority = field_id(&with.json, "Priority");
    let high = with.json["fields"].as_array().unwrap().iter().find(|f| f["name"] == "Priority").unwrap()["options"][0]["id"].as_str().unwrap().to_string();
    for bad in [json!({"name": "estimate", "kind": "text"}), json!({"name": "Labels", "kind": "text"}), json!({"name": "Size", "kind": "single_select"})] {
        assert_eq!(send(&app, POST, &fields, Some(&alice), Some(bad)).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    let filled = set(1, json!({"values": {&estimate: 3.5, &due: "2027-02-01", &notes: "ask bob", &priority: high}})).await;
    assert_eq!(filled.status, StatusCode::OK, "{}", filled.json);
    let row = filled.json["items"].as_array().unwrap().iter().find(|i| i["issue"]["number"] == 1).unwrap();
    assert_eq!((row["values"][&estimate].as_f64(), row["values"][&due].as_str(), row["values"][&notes].as_str()), (Some(3.5), Some("2027-02-01"), Some("ask bob")));
    for bad in [json!({&estimate: "three"}), json!({&due: "01/02/2027"}), json!({&notes: 5}), json!({&priority: "nope"}), json!({"ghost": "x"})] {
        assert_eq!(set(1, json!({"values": bad})).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    // Removing an option clears it from items; removing a field removes its values.
    let fewer = json!({"options": [{"name": "Urgent"}]});
    let changed = send(&app, PATCH, &format!("{fields}/{priority}"), Some(&alice), Some(fewer)).await;
    assert_eq!(changed.status, StatusCode::OK, "{}", changed.json);
    let row = changed.json["items"].as_array().unwrap().iter().find(|i| i["issue"]["number"] == 1).unwrap();
    assert!(row["values"].get(&priority).is_none(), "{row}");
    assert_eq!(send(&app, DELETE, &format!("{fields}/status"), Some(&alice), None).await.status, StatusCode::UNPROCESSABLE_ENTITY);
    let gone = send(&app, DELETE, &format!("{fields}/{notes}"), Some(&alice), None).await;
    let row = gone.json["items"].as_array().unwrap().iter().find(|i| i["issue"]["number"] == 1).unwrap();
    assert!(row["values"].get(&notes).is_none() && row["values"].get(&estimate).is_some());

    // The issue sidebar sees its projects and status.
    let mine = send(&app, GET, &format!("{repo}/issues/2/projects"), Some(&bob), None).await;
    assert_eq!((mine.json[0]["title"].as_str(), mine.json[0]["status"]["name"].as_str()), (Some("Roadmap"), Some("In Progress")));

    // Removing an item, deleting an issue, closing and deleting the project.
    assert_eq!(send(&app, DELETE, &format!("{p}/items/{}", item[&4]), Some(&bob), None).await.status, StatusCode::OK);
    send(&app, DELETE, &format!("{repo}/issues/3"), Some(&alice), None).await;
    let left = send(&app, GET, &p, Some(&bob), None).await;
    assert_eq!(left.json["item_count"], 2);
    let closed = send(&app, PATCH, &p, Some(&alice), Some(json!({"closed": true, "title": "Roadmap 2027"}))).await;
    assert_eq!((closed.json["closed"].as_bool(), closed.json["title"].as_str()), (Some(true), Some("Roadmap 2027")));
    assert_eq!(send(&app, DELETE, &p, Some(&bob), None).await.status, StatusCode::FORBIDDEN);
    assert_eq!(send(&app, DELETE, &p, Some(&alice), None).await.status, StatusCode::NO_CONTENT);
    assert_eq!(st.db.project_items.count_documents(mongodb::bson::doc! {}).await.unwrap(), 0);

    drop_db(&st).await;
}
