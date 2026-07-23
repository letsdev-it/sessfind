# Technical Specification

This directory describes how this codebase implements the product contract
in `letsdev-it/sdd-specs/`. Product behavior belongs in the spec repository;
this directory records implementation choices and conventions.

## Status

The repository is being bootstrapped. Add focused documents here as the
implementation stack and architecture are selected. Update the relevant tech
spec document in the same pull request as architectural code changes.

## Architecture

- [VS Code extension and JSON API](vscode-extension.md)

## Stack and dependencies

_To be defined._

## Data and interfaces

- The CLI is the system boundary for frontends. Machine-readable clients first
  inspect `sessfind capabilities`, then consume additive JSON shapes from
  `sessfind-common`.
- Native session IDs are source-scoped. Persistent metadata and frontend state
  use `<source>:<session_id>` as the stable identity.

## Development and operations

_To be defined._
