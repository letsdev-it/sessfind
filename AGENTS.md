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
