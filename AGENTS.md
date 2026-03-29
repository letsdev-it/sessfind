# Guidelines for AI assistants and contributors

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
