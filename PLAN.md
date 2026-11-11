# Plan: GitHub Issue Tracker clone

GitHub-এর issue tracker অংশের pixel-perfect clone। Backend Rust (Actix) + MongoDB,
frontend Next.js + Tailwind + shadcn, সব মিলে একটাই exe। সাথে `gh issue` এর মতো Rust CLI।

Architecture `../ninjatv` আর `../payment_processor` এর মতোই: Actix + MongoDB +
Next.js static export, `rust-embed` দিয়ে এক binary।

## 1. Architecture

Cargo workspace, তিনটা crate আর একটা frontend:

| অংশ | কাজ |
|---|---|
| `crates/server` | Actix API + embedded frontend, এটাই single exe |
| `crates/cli` | `gh issue ...` এর মতো CLI (binary নাম প্রস্তাব: `it`) |
| `crates/shared` | API request/response type, server আর CLI দুজনেই ব্যবহার করবে |
| `frontend/` | Next.js 16 + Tailwind 4 + shadcn, `output: "export"` |

- **Routing সমস্যা:** static export-এ `/owner/repo/issues/123` এর মতো অজানা dynamic path
  আগে থেকে build হয় না। সমাধান: Actix অজানা path-এ SPA shell ফেরত দেবে, client path পড়ে
  data আনবে।
- **Pixel-perfect এর উপায়:** GitHub-এর নিজের design system Primer (color token, spacing,
  typography) আর Octicons, দুটোই MIT license। এই token গুলো Tailwind theme-এ বসবে, shadcn
  component গুলো Primer-এর মাপে restyle হবে।
- **Branding:** GitHub-এর logo বা নাম ব্যবহার হবে না, কারণ ওগুলো trademark। Layout আর
  design হুবহু থাকবে, logo নিজের।

## 2. Feature scope

### Issue list page
- Open/Closed tab ও count, search bar-এ GitHub-এর query syntax
  (`is:open label:bug author:x sort:updated-desc`)
- Filter dropdown: Author, Labels, Projects, Milestones, Assignees, Types, Sort
- Row: state icon, title, label chip, `#12 opened 3 days ago by x`, assignee avatar,
  comment count
- Bulk select করে mark/label/assign, pagination, empty state

### Issue detail page
- Title edit, state badge, sticky header
- Comment timeline: markdown body, edit/delete, edit history, reactions, quote reply
- Timeline event: labeled, assigned, closed, reopened, renamed, cross-reference
  (`#12` mention), locked, pinned
- Sidebar: Assignees, Labels, Type, Milestone, Relationships (sub-issue/parent),
  Participants, Notifications
- Close as completed / not planned / duplicate, reopen, lock, pin, delete, transfer

### New issue page
- Markdown editor (Write/Preview tab, toolbar, `@mention` ও `#issue` autocomplete,
  image paste/upload)
- Issue template chooser

### অন্যান্য
- Labels page (CRUD, color picker), Milestones page (progress bar, due date)
- Markdown: GFM, task list, code highlight, emoji shortcode, server-side sanitize
- Light/Dark theme, পুরো responsive (mobile-এ sidebar নিচে নামে, filter গুলো dialog হয়)

## 3. Backend

- **Collection:** `users`, `repos`, `issues`, `comments`, `timeline_events`, `labels`,
  `milestones`, `reactions`, `attachments`, `tokens`, `counters` (repo প্রতি issue number)
- **API:** GitHub REST API-র shape অনুসরণ (`/api/repos/{owner}/{repo}/issues` ইত্যাদি)।
  এতে CLI আর পরে কোনো import tool সহজ হয়।
- **Search:** query string parser `shared` crate-এ, MongoDB filter-এ রূপান্তর, title/body-তে
  text index।
- **Auth:** web-এ session cookie, CLI-র জন্য personal access token।
- **Attachment:** MongoDB GridFS-এ, যাতে exe-র বাইরে কোনো ফোল্ডার না লাগে।

## 4. CLI (`gh issue` এর সমান)

`auth login/status/logout`,
`issue list/view/create/edit/close/reopen/comment/delete/pin/unpin/lock/unlock/transfer/status`,
`label list/create/edit/delete`, `milestone ...`

- `gh`-এর মতো flag: `--label`, `--assignee`, `--state`, `--search`, `--json`, `--web`
- `$EDITOR`-এ body লেখা, terminal-এ markdown render, রঙিন table
- Config `~/.config/it/config.toml`-এ (host + token), একাধিক host সাপোর্ট

## 5. Phase

| Phase | কাজ | শেষ হলে যা চলবে |
|---|---|---|
| 0 | Workspace, embed pipeline, Primer token, app shell (header, repo nav) | এক exe চালালে খালি shell দেখা যায় |
| 1 | Auth, repo, issue CRUD, list page (tab, pagination) | issue তৈরি ও দেখা |
| 2 | Detail page, comment, markdown editor, timeline event | পুরো কথোপকথন |
| 3 | Label, milestone, assignee, sidebar, filter dropdown, search syntax | filter আর search |
| 4 | Reactions, sub-issue, pin/lock/transfer, template, attachment, bulk action | বাকি feature |
| 5 | CLI | terminal থেকে সব কাজ |
| 6 | Responsive ও dark mode polish, GitHub-এর সাথে পাশাপাশি screenshot মিলিয়ে pixel ঠিক করা | চূড়ান্ত |

Testing browser automation দিয়ে, প্রতি phase শেষে আসল GitHub page-এর সাথে পাশাপাশি তুলনা করে।

## 6. চূড়ান্ত সিদ্ধান্ত (11 Nov 2026)

1. **Repo কাঠামো:** GitHub-এর মতো একাধিক `owner/repo`, URL `/owner/repo/issues`।
2. **User:** নিজস্ব username/password দিয়ে signup/login, প্রথম user admin, CLI-র জন্য
   personal access token।
3. **Permission:** repo প্রতি public/private আর collaborator role
   (read/triage/write/admin), GitHub-এর নিয়মে। Public repo login ছাড়া পড়া যাবে।
4. **Realtime:** SSE দিয়ে নতুন comment ও state পরিবর্তন refresh ছাড়াই page-এ আসবে।
5. **Notification inbox:** `/notifications` page, subscribe/mention/assign থেকে আসা।
6. **Email notification:** mention বা subscribed issue-তে email, SMTP config থেকে।
7. **Projects board:** GitHub Projects-এর table ও board view, custom field সহ।

### এর ফলে plan-এ যা যোগ হলো

- **Collection:** `collaborators`, `subscriptions`, `notifications`, `projects`,
  `project_items`, `project_fields`
- **Backend:** প্রতিটা endpoint-এ role check middleware, SSE endpoint (repo ও issue প্রতি
  channel), background email queue (`lettre`), SMTP না থাকলে email বন্ধ থাকবে
- **Frontend:** repo Settings → Collaborators page, visibility badge, notification inbox,
  Projects table/board (drag and drop)
- **CLI:** `repo create/list/view`, `project list/view/item-add`, `notification list`

### সংশোধিত Phase

| Phase | কাজ |
|---|---|
| 0 | Workspace, embed pipeline, Primer token, app shell |
| 1 | Auth, repo, public/private + role, issue CRUD, list page |
| 2 | Detail page, comment, markdown editor, timeline event |
| 3 | Label, milestone, assignee, sidebar, filter dropdown, search syntax |
| 4 | Reactions, sub-issue, pin/lock/transfer, template, attachment, bulk action |
| 5 | Realtime (SSE), subscription, notification inbox, email |
| 6 | Projects board (table view, board view, custom field) |
| 7 | CLI |
| 8 | Responsive ও dark mode polish, GitHub-এর সাথে পাশাপাশি pixel মিলানো |

## 7. অবস্থা (11 Nov 2026)

Phase 0 থেকে 8 সবগুলো তৈরি হয়েছে।

| Phase | অবস্থা | মন্তব্য |
|---|---|---|
| 0 | হয়েছে | এক binary-তে frontend embed, client router |
| 1 | হয়েছে | Signup/login, repo, public/private, role, issue CRUD, list |
| 2 | হয়েছে | Markdown (server-এ render ও sanitize), comment, edit history, timeline event, cross-reference |
| 3 | হয়েছে | Label, milestone, assignee, type, sidebar, filter dropdown, search syntax |
| 4 | হয়েছে | Reaction, sub-issue, pin, lock, transfer, template, attachment (GridFS), bulk action |
| 5 | হয়েছে | SSE live update, subscription, notification inbox, email queue |
| 6 | হয়েছে | Project: table view, board view (drag and drop), custom field |
| 7 | হয়েছে | `it` CLI, personal access token |
| 8 | আংশিক | Responsive ও dark mode যাচাই হয়েছে; issue list, issue detail, labels, milestones, projects list আর project table/board আসল GitHub-এর মাপ নিয়ে মেলানো; signed-in header, notifications, settings মেলানো হয়নি |

### যা এখনো বাকি বা ইচ্ছা করে বাদ

- Issue list, issue detail, labels, milestones, projects list আর project-এর table ও board view-এর মাপ
  (heading, row grid, font, padding, sidebar) আসল GitHub-এর public page থেকে নিয়ে বসানো হয়েছে, signed-out
  অবস্থায় দেখা page থেকে। Signed-in header, notifications আর settings page GitHub-এ login ছাড়া দেখা যায় না,
  তাই ওগুলো মাপ মিলিয়ে দেখা হয়নি; ওগুলো Primer token থেকে বানানো।
- Project page-এ GitHub-এর filter bar আর একাধিক saved view নেই; Table আর Board দুটো স্থির tab।
- Static release binary (`dist/issue-tracker` 23 MB, `dist/it` 6 MB) একা চালিয়ে যাচাই হয়েছে।
- Email পাঠানো আসল SMTP server দিয়ে পরীক্ষা হয়নি; test-এ শুধু queue পর্যন্ত যাচাই হয়েছে।
- Avatar upload নেই, login থেকে তৈরি identicon দেখায়।
- Organization নেই, repo-র মালিক শুধু user।
- Task list-এর checkbox page থেকে টিক দেওয়া যায় না, comment edit করে বদলাতে হয়।
- Search-এ `mentions:`, `commenter:`, `involves:`, `project:` qualifier নেই।
- Password reset আর email যাচাই নেই।
- Deployment target ঠিক হয়নি, তাই `deploy/` script নেই।
