# Issue Tracker — project guide

Clone of GitHub's issue tracker (issues, labels, milestones, notifications, projects),
self-hosted as one executable, plus a `gh issue`-style CLI. Talk to the user in Bangla.

- Plan, decisions and status: `PLAN.md` (Bangla). All phases 0–8 are built.
- Same architecture as `../ninjatv/` (Actix + MongoDB + Next.js static export in one binary).
- No GitHub logo or name anywhere: layout and design match, branding is our own.

## Layout

| Path | What |
|---|---|
| `crates/server/` | Actix server, binary `issue-tracker`. `routes/` = `/api`, `access.rs` = roles, `notify.rs` = inbox/email/live signal, `markdown.rs` = render + sanitize, `static_files.rs` = embedded frontend |
| `crates/server/tests/` | API tests against a real MongoDB (`api.rs`, `features.rs`) |
| `crates/cli/` | CLI, binary `it` (`main.rs` = arguments, `cmd.rs` = commands, `config.rs` = hosts and tokens) |
| `crates/shared/` | API request/response types and the search query parser, used by server and CLI |
| `frontend/` | Next.js 16 / Tailwind 4 / shadcn (Base UI), static export embedded by `rust-embed` |
| `scripts/` | `build-frontend.sh`, `release.sh` (static musl binaries into `dist/`), `seed-dev.sh` (demo data) |

## Rules that must not break

- A private repository the caller has no role in answers **404**, never 403, everywhere
  (API, attachments, markdown preview, live stream, notifications). Go through `access::load_repo`.
- Roles: read < triage < write < admin. Site admin and the owner are admin. A signed-in
  stranger has read on a public repository. What each role may do is in `access.rs` and
  at the top of each route file; the frontend only hides buttons, the server decides.
- Markdown is rendered on the server and always passes through ammonia (`markdown.rs`).
  The frontend inserts `body_html` as is, so nothing may bypass that path.
- Uploads are sniffed: only real PNG/JPEG/GIF/WebP are served inline, everything else is a
  download. Downloads re-check repository access.
- Session cookie values and access tokens are stored as SHA-256 only. Passwords are argon2.
- Access tokens cannot manage tokens (`Viewer::require_session`).
- The search syntax is parsed in `crates/shared/src/query.rs` and mirrored in
  `frontend/lib/query.ts`; change both together.
- `notify.rs` must never fail the request that triggered it.

## How the frontend is served

- The export has one page (`app/page.tsx`). The server answers every non-file, non-`/api`
  path with `index.html`; `frontend/lib/router.tsx` matches the path on the client.
- Add a page by adding a `Route` variant in `lib/router.tsx` and rendering it in
  `components/app.tsx`. Do not add Next file routes.
- Navigate with `Link` / `navigate` from `lib/router.tsx`, not `next/link`.
- Data comes through SWR with the API path as the key; `lib/live.ts` refetches keys when
  the server-sent event stream says something changed.
- Colors, sizes and fonts come from Primer tokens (`@primer/primitives`), exposed as Tailwind
  utilities in `app/globals.css` (`text-fg-muted`, `bg-canvas-inset`, `border-line`). Icons are
  `@primer/octicons-react`. No raw hex values in components.
- Breakpoints are Primer's: sm 544, md 768, lg 1012, xl 1280.
- A component passed as `trigger` to `SelectPanel` / `ActionMenu` must spread the props it
  receives onto its button, or the panel will not open.

## Local development

- Node: `~/.nvm/versions/node/v24.19.0/bin` (the default `node` on PATH is v12 and will not work).
- MongoDB: container `it-mongo` (mongo:7) on `127.0.0.1:27020`; tests create and drop
  `it_test_*` databases there.
- Run: `./scripts/build-frontend.sh`, then `cargo run -p issue-tracker` (debug builds read
  `frontend/out` from disk). Server listens on `127.0.0.1:23500`. Settings: `.env.example`.
- Demo data: `./scripts/seed-dev.sh` (users `demo`, `alice`, `bob`; password in the script).
- CLI: `cargo run -p it -- --help`.
- Tests: `cargo test` (needs `it-mongo` running).
- Build outputs stay on this drive (`target/`, `frontend/node_modules`); the home drive is full.
- Testing is Rust tests + browser automation only. No Playwright or similar specs.
- Deployment target is not decided yet.
