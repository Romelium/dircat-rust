# Commit Message Guidelines

This project follows the [Conventional Commits specification (v1.0.0)](https://www.conventionalcommits.org/en/v1.0.0/). Adhering to these guidelines improves commit history clarity and enables potential automation like changelog generation.

## Format

Each commit message consists of a **header**, a **body**, and a **footer**.

```plaintext
<type>(<scope>): <subject>
<BLANK LINE>
<body>
<BLANK LINE>
<footer>
```

* **Header:** Contains the `type`, optional `scope`, and `subject`.
  * `<type>`: See allowed types below.
  * `(<scope>)`: Optional. See defined scopes below.
  * `<subject>`: Concise description of the change.
* **Body (Optional):** Crucial for non-trivial changes. Explain the problem the commit solves and *why* the change is needed, not just *what* changed. Provide context, motivation, and potentially outline the approach taken or alternatives considered. Use the imperative, present tense.
* **Footer (Optional):** Contains information about breaking changes or references to issues.

### Commit Types (`<type>`)

Must be one of the following:

* **`feat`**:
  * Introduces a new feature or functionality visible to the user.
* **`fix`**:
  * Corrects a bug in the application code, fixing incorrect behavior visible to the user.
* **`refactor`**:
  * Improves internal code structure or implementation (e.g., simplifying logic, improving readability) without changing its user-observable behavior.
  * Does not fix a bug or add a feature. Primarily impacts maintainability.
  * Note: Do not use `refactor` if the primary motivation is performance; use `perf` instead.
* **`perf`**:
  * Improves application performance without changing functionality.
  * Use this even if the change involves refactoring, if performance is the primary motivation.
  * Usually requires a scope (e.g., `perf(parser): ...`).
* **`style`**:
  * Changes that do not affect the meaning or execution of the code (e.g., white-space, formatting, fixing linter warnings).
  * Often does not require a scope (e.g., `style: run cargo fmt`).
* **`test`**:
  * Adding new tests or correcting existing tests for application code, verifying its behavior and correctness.
  * Improves test coverage or correctness.
  * Use the scope of the code being tested (e.g., `test(parser): ...`).
  * Do not use this type for changes *only* to test infrastructure/helpers. For those, use `fix`/`feat`/`refactor` with the `test_infra` scope.
* **`build`**:
  * Changes affecting the build system or external dependencies (e.g., modifying `Cargo.toml`).
  * `Cargo.lock` updates usually accompany `build` commits when dependencies change.
  * Note: For dependency updates that *only* change `Cargo.lock` (e.g., after `cargo update`), use the `chore` type instead.
* **`ci`**:
  * Changes to CI configuration files and scripts (e.g., `.pre-commit-config.yaml`, GitHub Actions).
* **`docs`**:
  * Documentation-only changes (e.g., `README.md`, code comments).
  * Often does not require a scope.
* **`chore`**:
  * Other changes that do not modify application code in `src/` or test code, and do not affect the user-facing functionality or fix bugs (e.g., updating `.gitignore`, tooling configuration, dependency updates only affecting `Cargo.lock`).
  * While often unscoped, `chore` can take a scope if relevant (e.g., `chore(ci): update pre-commit hook versions`).
* **`revert`**:
  * Reverts a previous commit.
  * The header should be `revert: <header of commit being reverted>`.
  * The body must state `This reverts commit <hash>.` and should also explain *why* the commit is being reverted.

### Subject Line (`<subject>`)

* Use the imperative, present tense (e.g., "add", "fix", "change" not "added", "fixed", "changed"). Think of the commit message as commanding the codebase to perform the change.
* Start with a lowercase letter. This is the recommended convention for Conventional Commits.
* Do not end the subject line with a period.
* Keep it concise (aim for under 50 characters for readability in Git logs and UIs, but this is a guideline, not a hard rule).

### Footer Content

* **Breaking Changes:** Start with `BREAKING CHANGE:` followed by a short summary of the breaking change. The commit *body* must provide a detailed explanation of the change, justification, and migration notes. A `!` can also be appended to the type/scope (e.g., `feat(parser)!:`) to draw attention to a breaking change in the header.
* **Issue References:** Use keywords like `Closes #123`, `Fixes #456`, `Refs #789`.

## Defined Scopes (`<scope>`)

Choose the most specific scope that represents the primary area of impact for the change.

**Error Handling Scope:** Changes primarily defining/refactoring general error types in `src/errors.rs` use the `core` scope. Changes involving adding/fixing/handling errors *within* specific modules (`parser`, `processor`, `cli`) should use that module's scope.

The following scopes are defined for this project:

* **`parser`**: Changes related to parsing the input markdown file (`src/parser/`).
* **`processor`**: Changes related to processing actions and interacting with the filesystem (`src/processor/`).
* **`core`**: Changes to fundamental library elements (`src/core_types.rs`, `src/constants.rs`, `src/lib.rs`, `src/errors.rs` for general error types).
* **`cli`**: Changes related to the command-line interface executable (`src/main.rs`).
* **`test_infra`**: Changes *only* affecting the test harness, setup, or shared helpers (e.g., files within `tests/common/`). Use this scope with types like `fix`, `feat`, or `refactor`. Do not use this scope when adding or modifying tests for specific application code (use the `test` *type* and the relevant code scope instead, e.g., `test(parser): ...`).
* **`ci`**: Changes to CI/CD configuration or pre-commit hooks (`.pre-commit-config.yaml`, `.github/workflows/`).
* **`build`**: Changes to the build system or dependencies (`Cargo.toml`).

## Examples

* `feat(parser): add support for internal comment headers`
* `fix(processor): prevent overwriting files without --force flag`
* `fix(parser): handle unclosed code fences gracefully`
* `feat(core): define general AppError enum in errors.rs`
* `refactor(core): simplify Error enum variants`
* `refactor(core): rename ActionType variants for clarity`
* `refactor(parser): simplify header extraction logic`
* `perf(parser): optimize regex matching for large files`
* `test(parser): add integration tests for nested blocks`
* `feat(test_infra): add helper function for temp dir setup`
* `fix(test_infra): correct assertion logic in common test helper`
* `chore(ci): update pre-commit hook versions`
* `docs: update README with setup instructions`
* `style: run cargo fmt`
* `build: add 'thiserror' dependency to Cargo.toml`
* `chore: update dependencies via cargo update`
* `chore: add target directory to .gitignore`
* `fix(cli): correct default output directory path`

* ```plaintext
    revert: feat(parser): add support for internal comment headers

    This reverts commit a1b2c3d4e5f6 because the new header format
    introduced compatibility issues with older documents.

    This reverts commit a1b2c3d4e5f67890abcdef1234567890abcdef.
