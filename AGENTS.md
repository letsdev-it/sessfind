# Guidelines for AI assistants and contributors

## Product specification and SDD

The product contract for `sessfind` lives in the `letsdev-it/sdd-specs`
repository. Treat the current product spec as authoritative for observable
behavior.

- Contract changes start as `spec-feature` or `spec-chore` issues in the spec
  repository and are implemented only after the resulting code task exists.
- Bugs and maintenance work start as `code-bug` or `code-chore` issues in the
  spec repository.
- Every pull request must close or reference its generated `sdd:task` issue.
- Do not introduce observable behavior that is absent from, or contradicts,
  the current product spec. Route contract changes through the spec first.
- Update the relevant technical-spec document under `spec/` in the same pull
  request when the implementation architecture or conventions change.

## Product clarification

When implementation discovers a missing or ambiguous **product decision**,
do not invent behavior in code. Add this comment to the open `sdd:task` issue
in this code repository (not to the PR and not directly to the spec repo):

```text
/sdd clarify <focused product question>
```

Valid product questions include:

```text
/sdd clarify Should sessions without a project be included in an unfiltered list?
/sdd clarify What should the CLI display when a stored session source is no longer installed?
/sdd clarify Should deleting an indexed source also remove its cached semantic data?
```

Do not use clarification for implementation choices:

```text
/sdd clarify Should this use an enum or a trait object?
/sdd clarify Which Rust crate should implement date parsing?
/sdd clarify How should this module be split?
```

Those are technical decisions owned by this repository and may be recorded in
its technical spec or an implementation ADR. Clarification keeps the existing
task and branch, marks the task `sdd:blocked-product`, and creates a focused
draft spec PR. Resume after the product decision is merged and an approved
`update` operation removes the blocked label.

## Completion authority

An agent must not complete an SDD task by moving a Project card, editing
labels, posting a completion comment, or closing the issue manually. A task
may reach terminal state only through:

- a merged PR using `Closes #<task>` after task-link, conformance, fulfillment,
  and spec-freshness are all successful; or
- an approved `supersede` operation from a merged product-spec impact plan.

Unauthorized issue closure is automatically reverted.

## Commits

Use **[Conventional Commits](https://www.conventionalcommits.org/)** so tooling (e.g. release-plz) can infer semver and changelogs.

- Format: `<type>(<scope>): <description>` — scope is optional.
- Common types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`, `ci`, `perf`.
- Use imperative mood in the subject (`add`, `fix`, not `added`, `fixed`).
- Breaking changes: add `!` after the type (e.g. `feat!: remove old CLI flag`) or a `BREAKING CHANGE:` footer in the body.

Examples:

- `feat(tui): add keyboard shortcut for clearing search`
- `fix(indexer): skip invalid session files`
- `chore: bump dependency patch versions`

## Documentation

Always update `README.md` after making user-facing changes (new flags, features, behavior changes, etc.).

When adding or removing a page in `docs/`, update the `nav` section in `mkdocs.yml` to keep the documentation site in sync.

## Before push

Before pushing a branch, run the same checks that CI runs for pull requests:

- `cargo fmt --all -- --check`
- `cargo build --workspace`
- `cargo test --workspace`
