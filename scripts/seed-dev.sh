#!/usr/bin/env bash
# Fills a running dev server with demo data. Local development only: the accounts
# below are throwaway test accounts. Drop the dev database first for a clean slate:
#   docker exec it-mongo mongosh --quiet issue_tracker --eval 'db.dropDatabase()'
set -euo pipefail
B="${IT_HOST:-http://127.0.0.1:23500}/api"
PASS="demo-pass-1234"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

call() { # call <user> <method> <path> [json]
  local user=$1 method=$2 path=$3
  if [ $# -ge 4 ]; then
    curl -sS -b "$TMP/$user" -X "$method" -H 'Content-Type: application/json' -d "$4" "$B$path"
  else
    curl -sS -b "$TMP/$user" -X "$method" "$B$path"
  fi
}

for user in demo alice bob; do
  curl -sS -o /dev/null -c "$TMP/$user" -H 'Content-Type: application/json' \
    -d "{\"login\":\"$user\",\"email\":\"$user@example.test\",\"password\":\"$PASS\"}" "$B/auth/signup"
  curl -sS -o /dev/null -c "$TMP/$user" -H 'Content-Type: application/json' \
    -d "{\"login\":\"$user\",\"password\":\"$PASS\"}" "$B/auth/login"
done

call demo POST /user/repos '{"name":"tracker","description":"Demo repository for the issue tracker"}' >/dev/null
call demo POST /user/repos '{"name":"secret-plans","description":"Private demo repository","private":true}' >/dev/null
call demo PUT /repos/demo/tracker/collaborators/alice '{"role":"write"}' >/dev/null
R=/repos/demo/tracker

titles=(
  "Login page throws 500 when email has uppercase letters"
  "Add dark mode toggle to the user menu"
  "Search box loses focus after typing a slash"
  "Pagination links wrap badly on small phones"
  "Support pasting images into the comment box"
  "Typo in the empty state message"
  "Sidebar overlaps the timeline at 768px"
  "Allow closing an issue as duplicate"
  "CLI: add --json flag to issue list"
  "Notification bell shows a stale count"
)
authors=(demo alice bob)
for i in $(seq 1 32); do
  t="${titles[$((i % 10))]}"
  call "${authors[$((i % 3))]}" POST $R/issues "{\"title\":\"$t\",\"body\":\"Steps to reproduce:\\n\\n1. Open the page\\n2. Look at item $i\\n\\nExpected: it works.\"}" >/dev/null
done

BODY=$(cat <<'MD'
## What happens

Signing in with `Demo@Example.test` answers **500**. Lowercase works. Related to #2, ping @alice.

```rust
fn normalise(email: &str) -> String {
    email.trim().to_lowercase() // missing on the login path
}
```

- [x] Reproduce locally
- [ ] Add a regression test

| Input | Result |
|---|---|
| `demo@example.test` | ok |
| `Demo@Example.test` | 500 |

> Seen on the staging server too. :bug:
MD
)
call demo POST $R/issues "$(printf '%s' "$BODY" | python3 -c 'import json,sys; print(json.dumps({"title":"Login fails for mixed-case email addresses","body":sys.stdin.read()}))')" >/dev/null
N=33
call alice POST $R/issues/$N/comments '{"body":"I can reproduce this. The lookup uses the raw value, see the snippet above.\n\nI will take it."}' >/dev/null
call bob POST $R/issues/$N/comments '{"body":"Same root cause as #1?"}' >/dev/null
call demo PATCH $R/issues/$N '{"title":"Login fails for mixed-case email addresses (500)"}' >/dev/null
call demo POST $R/issues/$N/comments '{"body":"Fixed on main, closing."}' >/dev/null
call demo PATCH $R/issues/$N '{"state":"closed"}' >/dev/null
call demo PATCH $R/issues/$N '{"state":"open"}' >/dev/null

# Organise: milestones, labels, types, assignees, a pinned issue, sub-issues, reactions, templates.
call demo POST $R/milestones '{"title":"v1.0","description":"First public release","due_on":"2027-01-31T00:00:00Z"}' >/dev/null
call demo POST $R/milestones '{"title":"v1.1","description":"Polish"}' >/dev/null
labels=('["bug"]' '["enhancement"]' '["bug","help wanted"]' '["bug","good first issue"]' '["enhancement","question"]' '["documentation"]' '[]')
types=('"Bug"' '"Feature"' '"Bug"' '"Task"' 'null')
people=('["alice"]' '[]' '["demo","alice"]' '[]')
for i in $(seq 1 33); do
  ms=null; [ $((i % 3)) -eq 0 ] && ms=1; [ $((i % 7)) -eq 0 ] && ms=2
  call demo PATCH $R/issues/$i "{\"labels\":${labels[$((i % 7))]},\"issue_type\":${types[$((i % 5))]},\"assignees\":${people[$((i % 4))]},\"milestone\":$ms}" >/dev/null
done
call demo PUT $R/issues/33/pin >/dev/null
call demo POST $R/issues/33/sub_issues '{"number":30}' >/dev/null
call demo POST $R/issues/33/sub_issues '{"number":20}' >/dev/null
call alice PUT $R/issues/33/reactions/+1 >/dev/null
call bob PUT $R/issues/33/reactions/+1 >/dev/null
call bob PUT $R/issues/33/reactions/rocket >/dev/null
call demo PUT $R/issue-templates '[{"name":"Bug report","about":"Something is not working","title":"[Bug]: ","body":"## What happened\n\n## Steps to reproduce\n\n1. \n\n## Expected\n","labels":["bug"],"issue_type":"Bug"},{"name":"Feature request","about":"Suggest an idea","body":"## Problem\n\n## Proposal\n","labels":["enhancement"],"issue_type":"Feature"}]' >/dev/null

# A project with a few cards spread over the status columns and one custom field.
P=$(call demo POST $R/projects '{"title":"Release 1.0","description":"What ships in the first release"}')
opt() { printf '%s' "$P" | python3 -c "import json,sys; print([o['id'] for o in json.load(sys.stdin)['fields'][0]['options'] if o['name']=='$1'][0])"; }
TODO=$(opt Todo); DOING=$(opt "In Progress"); DONE=$(opt Done)
call demo POST $R/projects/1/fields '{"name":"Estimate","kind":"number"}' >/dev/null
col=("$TODO" "$DOING" "$DONE" "$TODO" "")
for n in 33 30 28 27 24 23 21 17; do
  item=$(call demo POST $R/projects/1/items "{\"issue_number\":$n}" | python3 -c "import json,sys; print([i['id'] for i in json.load(sys.stdin)['items'] if i['issue']['number']==$n][0])")
  s="${col[$((n % 5))]}"
  [ -n "$s" ] && call demo PATCH $R/projects/1/items/$item "{\"values\":{\"status\":\"$s\"}}" >/dev/null
done

for n in 3 7 12; do call demo PATCH $R/issues/$n '{"state":"closed"}' >/dev/null; done
call demo PATCH $R/issues/20 '{"state":"closed","state_reason":"not_planned"}' >/dev/null
echo "seeded: users demo, alice, bob (password in this script); repositories demo/tracker, demo/secret-plans"
