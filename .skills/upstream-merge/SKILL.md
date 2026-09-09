---
name: upstream-merge
description: Synchronize the current branch with the repository's upstream branch, classify functional conflicts for user choice, merge safely, and verify frontend and backend compilation.
---

# Upstream merge workflow

Use this skill when the user asks to synchronize upstream changes or merge an upstream branch into a local feature branch.

## Repository-specific fork contract

For this Aether fork, read [`FORK_DELTA.md`](./FORK_DELTA.md) after fetching and before classifying or resolving conflicts. Refresh its pre-merge refs, divergence counts, overlap paths, and pending-upstream section for the current merge. Treat its P0 invariants as required review points, but still ask the user before resolving functional conflicts. This pre-merge refresh does not replace the mandatory post-merge review below.

## Safety rules

- Inspect `git status --short --branch` before changing anything. Never discard local work.
- Identify the upstream repository from an existing `upstream` remote, repository metadata, or the project README. If no reliable upstream can be identified, ask the user.
- Fetch first, then compare `HEAD...upstream/main` (or the requested upstream branch).
- If the worktree is dirty, stop and ask whether to stash/commit/continue; do not silently overwrite changes.
- **Batching rule for large merges**: For large upstream merges (over 40 commits or containing structural refactorings like database removals or credential migrations), split the merge into batches. Choose split points right before structural refactoring commits. Merge, verify, and commit each batch separately.
- Start the merge with `git merge --no-commit --no-ff` so conflicts can be reviewed before committing.
- On conflicts, do not choose `ours` or `theirs` automatically. Group conflicts by behavior (not only by filename) and explain each side's functional effect. Ask the user to choose per group.

## Conflict classification

For each conflict group, inspect both stages with:

```sh
git diff --cc -- path
git show :2:path   # local/ours
git show :3:path   # upstream/theirs
```

Describe choices as `ours`, `theirs`, or `manual hybrid`. Highlight effects on APIs, database schemas/migrations (PostgreSQL only; MySQL and SQLite have been removed), background workers, scheduling, authentication, billing/usage, and UI behavior. Treat rename/delete conflicts as migration decisions, not merely file conflicts.

Record the user's choices in a compact mapping such as `1C, 2B, 3C` before resolving files. For a hybrid choice, preserve both behaviors while adopting shared infrastructure from upstream when possible.

## Resolution and verification

- Resolve only after the user chooses. Use `git add` to mark resolved paths and verify `git diff --name-only --diff-filter=U` is empty.
- Run `git diff --check` before committing.
- Detect build commands from `frontend/package.json` and the workspace manifests. For this repository use:

```sh
cd aether-vscodex/web && npm install   # only when the VSCodex web dependencies are absent
cd frontend && npm install             # only when the main frontend dependencies are absent
cd frontend && npm run build
CARGO_BUILD_JOBS=1 cargo check --workspace
```

- The frontend build and `cargo check --workspace` are the mandatory compile baseline. Do not expand them into the full `FORK_DELTA.md` behavior-test checklist by default.
- Run behavior tests only for the files and contracts that actually conflicted or required a merge-regression fix. Choose the smallest focused command; a frontend-only conflict should not trigger unrelated Rust suites.
- Never run multiple Cargo build/check/test processes concurrently. Cargo shares package-cache and artifact locks, and parallel invocations can serialize behind expensive duplicate compilation. Run at most one targeted Cargo command at a time.
- “只允许一个rust编译进程”：全程禁止并行 Rust 编译；所有 Cargo build/check/test 命令必须串行执行，并设置 `CARGO_BUILD_JOBS=1`，确认上一条命令已退出后才能启动下一条。
- Treat the behavior commands in `FORK_DELTA.md` as a reference menu, not an automatic batch. Record any command not run as unverified; if the user stops jobs, cancel them immediately and continue only with evidence already obtained.
- If either build fails, classify whether it is a merge-resolution regression or an environment/dependency issue. Fix merge regressions in scope; do not claim completion until both compile successfully.
- Commit the merge only after both builds pass. Do not push unless the user explicitly requests it.
- After creating the verified merge commit, review the merged tree against upstream again and update `FORK_DELTA.md` in a separate documentation commit. Refresh the snapshot date, merged code commit, upstream commit, merge-base/counts, overlap paths, pending-upstream list, feature inventory, conflict rules, and verification commands. Record an explicit “no functional delta change” result when applicable; the review is mandatory even when no feature section changes.
- Keep the documentation refresh separate from the merge commit so `FORK_DELTA.md` can name the immutable merge commit without a self-referential amend changing its hash. Do not deliver or push until both commits exist and the refreshed document passes `git diff --check`.
- Final response must include the merge commit, the `FORK_DELTA.md` documentation commit, the post-merge delta review conclusion, selected conflict policy, frontend/backend verification results, and any non-blocking warnings (for example dependency audit findings).

## Suggested progress updates

1. Report branch/worktree/remotes and the upstream target.
2. Report that fetch and a no-commit merge are in progress.
3. If conflicts occur, pause with grouped functional choices.
4. Report resolution and build progress.
5. Report the mandatory post-merge `FORK_DELTA.md` review and documentation commit.
6. Deliver both commits and the verification summary.
