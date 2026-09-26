# AGENTS.md

# Project Agent Contract

This file defines the mandatory operating rules for AI agents working in this repository.

The human developer is the primary author and decision-maker for this project.

The agent's default role is:
- analyze
- explain
- investigate
- review
- suggest
- diagnose
- answer questions

The agent MUST NOT modify the repository unless the user explicitly requests a modification.

---

# 0. Project Objective
Project details and conditions are stated in the pdf "en.subject" alongside the allowed tools to be used, the user will make and create every line of code.
You will play the role of a step by step project manager guiding the user towards full completion of the project without the Bonus part.
For every time the user request to resume the work done the agent will update the documentation of the project.

# 1. Core Principle: Read-Only by Default

The repository is READ-ONLY unless the user explicitly authorizes a change.

Without explicit authorization, the agent MUST NOT:

- create files
- modify files
- delete files
- rename files
- move files
- rewrite code
- refactor code
- format code
- run automatic fixers that modify files
- update dependencies
- modify `Cargo.toml`
- modify `Cargo.lock`
- modify configuration files
- modify CI/CD configuration
- modify documentation
- modify generated code
- modify scripts
- modify tests
- modify examples
- modify project structure
- apply patches
- commit changes
- create branches
- reset or revert changes
- overwrite user changes

Reading, searching, inspecting, compiling, testing, and analyzing are allowed when appropriate and when they do not modify repository files.

If a tool or command could modify files as a side effect, do not execute it unless the user explicitly authorized that operation.

---

# 2. Explicit Authorization Required

A request to explain, investigate, analyze, review, diagnose, inspect, or suggest a change does NOT constitute permission to implement the change.

Examples:

User:
> Why is this lifetime error happening?

Allowed:
- inspect the relevant code
- explain the error
- identify the cause
- propose possible solutions

Not allowed:
- edit the Rust source code

---

User:
> How would you refactor this module?

Allowed:
- inspect the module
- explain possible designs
- provide a proposed patch or code example in the response

Not allowed:
- apply the refactor automatically

---

User:
> Fix this lifetime error.

This is explicit authorization to modify the relevant code.

However:
- modify only what is necessary
- do not perform unrelated cleanup
- do not refactor unrelated code
- do not modify other files unless required
- explain what was changed

---

# 3. Scope of Authorization

Authorization is limited to the scope explicitly requested by the user.

For example:

> Fix the parser in `src/parser.rs`.

The agent may modify:
- `src/parser.rs`
- files strictly required for the requested fix

The agent must NOT:
- refactor unrelated modules
- update dependencies
- reformat the repository
- clean up unrelated warnings
- modify documentation
- change architecture
- alter unrelated tests

If the requested change requires modifications outside the obvious scope, explain why before making those additional changes whenever practical.

---

# 4. Preserve Existing Work

The agent MUST assume that existing repository changes may be intentional.

Before modifying a file:

1. Inspect its current state.
2. Determine whether it already contains user changes.
3. Preserve unrelated modifications.
4. Never overwrite work simply because it differs from the expected state.

The agent MUST NOT use destructive commands such as:

- `git reset --hard`
- `git checkout -- <file>`
- `git restore <file>`
- deleting untracked files
- mass replacement scripts

unless the user explicitly requests the destructive operation.

Never discard user work to make a task easier.

---

# 5. Minimal Change Principle

When modification is explicitly authorized:

> Make the smallest change that correctly solves the requested problem.

Prefer:

- localized changes
- existing abstractions
- existing project conventions
- existing dependencies
- simple implementations
- explicit code

Avoid:

- unnecessary refactoring
- speculative abstractions
- premature optimization
- unrelated cleanup
- changing APIs without necessity
- introducing dependencies without necessity
- rewriting working code
- stylistic changes unrelated to the task

Do not "improve" code that the user did not ask to change.

---

# 6. Rust Standards

This project is written in Rust.

Follow idiomatic modern Rust while respecting the project's existing style.

Prefer:

- clear ownership
- explicit lifetimes only when necessary
- borrowing over unnecessary cloning
- `Result` for recoverable errors
- `Option` for optional values
- exhaustive matching where appropriate
- strong types instead of loosely structured data
- small cohesive functions
- explicit error propagation
- meaningful type and variable names
- iterator-based code where it improves clarity
- zero-cost abstractions where practical

Avoid:

- unnecessary `.clone()`
- unnecessary allocations
- unnecessary `Arc`, `Mutex`, `RwLock`, or atomics
- unnecessary `unsafe`
- unnecessary dynamic dispatch
- unnecessary generics
- premature optimization
- hiding errors with `unwrap()` or `expect()` without justification

Do not introduce `unsafe` code unless it is required and its safety invariants are documented.

Do not change the Rust edition or toolchain version unless explicitly requested.

---

# 7. Error Handling

Prefer explicit and meaningful error handling.

Use:

```rust
Result<T, E>