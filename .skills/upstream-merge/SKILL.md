---
name: upstream-merge
description: Synchronize the current branch with the repository's upstream branch, classify functional conflicts for user choice, merge safely, and verify frontend and backend compilation.
---

# Upstream merge workflow

Use this skill when the user asks to synchronize upstream changes or merge an upstream branch into a local feature branch.

## Safety rules

- Inspect `git status --short --branch` before changing anything. Never discard local work.
- Identify the upstream repository from an existing `upstream` remote, repository metadata, or the project README. If no reliable upstream can be identified, ask the user.
- Fetch first, then compare `HEAD...upstream/main` (or the requested upstream branch).
- If the worktree is dirty, stop and ask whether to stash/commit/continue; do not silently overwrite changes.
- Start the merge with `git merge --no-commit --no-ff` so conflicts can be reviewed before committing.
- On conflicts, do not choose `ours` or `theirs` automatically. Group conflicts by behavior (not only by filename) and explain each side's functional effect. Ask the user to choose per group.

## Conflict classification

For each conflict group, inspect both stages with:

```sh
git diff --cc -- path
git show :2:path   # local/ours
git show :3:path   # upstream/theirs
```

Describe choices as `ours`, `theirs`, or `manual hybrid`. Highlight effects on APIs, database schemas/migrations, background workers, scheduling, authentication, billing/usage, and UI behavior. Treat rename/delete conflicts as migration decisions, not merely file conflicts.

Record the user's choices in a compact mapping such as `1C, 2B, 3C` before resolving files. For a hybrid choice, preserve both behaviors while adopting shared infrastructure from upstream when possible.

## Resolution and verification

- Resolve only after the user chooses. Use `git add` to mark resolved paths and verify `git diff --name-only --diff-filter=U` is empty.
- Run `git diff --check` before committing.
- Detect build commands from `frontend/package.json` and the workspace manifests. For this repository use:

```sh
cd frontend && npm install   # only when dependencies are absent
cd frontend && npm run build
cargo check --workspace
```

- If either build fails, classify whether it is a merge-resolution regression or an environment/dependency issue. Fix merge regressions in scope; do not claim completion until both compile successfully.
- Commit the merge only after both builds pass. Do not push unless the user explicitly requests it.
- Final response must include the merge commit, selected conflict policy, frontend/backend verification results, and any non-blocking warnings (for example dependency audit findings).

## Suggested progress updates

1. Report branch/worktree/remotes and the upstream target.
2. Report that fetch and a no-commit merge are in progress.
3. If conflicts occur, pause with grouped functional choices.
4. Report resolution and build progress.
5. Deliver the commit and verification summary.
