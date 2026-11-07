# Taskmaster — Mandatory Scope & Current State

## 1. Goal (mandatory only)

Build a job-control supervisor (inspired by `supervisor`) in Rust.
Bonus features (client/server, privilege de-escalation, alerts, attach/detach)
are **out of scope** until the mandatory part is fully reliable.

The program must:

1. Load a configuration file at launch.
2. Spawn and keep alive the configured number of child processes per program.
3. Know, at all times and accurately, whether each child is alive or dead.
4. Apply the per-program restart policy (always / never / on-unexpected).
5. Enforce start-time, start-retries, stop-signal, stop-timeout, exit-codes.
6. Redirect/discard `stdout` and `stderr`; apply `env`, `workingdir`, `umask`.
7. Expose a foreground **control shell** with:
   `status`, `start`, `stop`, `restart`, `reload`, `quit`.
8. Log start/stop/restart/unexpected-death/reload events to a local file.
9. Reload the configuration on `SIGHUP` **without** despawning unchanged
   programs.

## 2. Repository layout

```
Taskmaster/
├── config-parser/   # TOML config model + validators
├── runtime/         # Process supervision (FFI scaffolding in progress)
├── shell/           # Control shell (placeholder)
├── taskmaster/      # Binary entry point
└── Run.toml         # Sample configuration
```

## 3. What is implemented

### 3.1 Configuration layer — mostly done

- Cargo workspace with four crates.
- `ConfigReader` loads a path, reports read errors, and refuses to parse
  until the file has been read successfully. Deserializes into any
  Serde-compatible type.
- `Config` and `ProgramConfig` structures model each supervised program.
- Field-level validators (in `config-parser/src/validators/`) for:
  - `Command` — rejects empty, non-existent, directory, and non-executable
    paths; splits into executable + args.
  - `Stdin` / `Stdout` output targets, implemented as a generic
    `OutputTarget<T>` where `T` is a role marker (compile-time safety).
    Deserialization uses `#[serde(try_from = "String")]`, so invalid paths
    fail during parsing.
  - Working directory, process count, umask (octal), restart policy, exit
    codes, stop signal, env-map shape.
- 39 passing unit tests in `config-parser`.

### 3.2 Runtime layer — FFI scaffolding only

Design decision: no third-party Rust process-management crate. All Unix
calls go through locally-declared FFI in a single `unix` module.

Checkpoints already landed in the runtime crate:

- `pipe2`, `close`, `read`, `write` FFI with a `SelfPipe` RAII wrapper
  (non-blocking, close-on-exec, both descriptors owned via `Drop`).
- Blocking `poll` wrapper on the read end; verified by test.
- `sigaction` structs and installer for Linux x86_64; test saves and
  restores the process-wide `SIGCHLD` disposition.

### 3.3 Not yet implemented

- `SIGCHLD` handler wired to the self-pipe write end.
- `waitpid(..., WNOHANG)` reap loop.
- Spawn/registration with `SIGCHLD` blocked around it.
- Single-process lifecycle (spawn → wait → structured exit result).
- Multi-process supervision, restart policy, restart limits.
- Graceful stop (configured signal) + forced kill after `stoptime`.
- Logging to file.
- Interactive control shell.
- `SIGHUP` reload with diff-based program updates.
- `stderr` field in the config model and `chmod` → `umask` rename.

## 4. Immediate next step

Wire a minimal `SIGCHLD` handler to the self-pipe write descriptor.

Constraints (async-signal-safe):

- No allocation, no `println!`, no locks, no panics inside the handler.
- Only a raw FFI `write(fd, &byte, 1)`.
- `fd` reached via a `static AtomicI32` set before `sigaction`.
- Ignore `EINTR` and `EAGAIN` on the write (pending byte = success).

Test: install handler, raise `SIGCHLD` from the test, assert the self-pipe
becomes readable and drains exactly one byte.

After that, in order:

1. `waitpid(-1, &status, WNOHANG)` reap loop drained on every `poll` wake.
2. Block `SIGCHLD` with `sigprocmask` around spawn + child registration so
   the handler never fires before the PID is recorded.
3. Single-process lifecycle in safe Rust using the mechanism above.
4. Multi-process supervision + restart policy.
5. Graceful stop and forced kill.
6. Logging.
7. Control shell.
8. `SIGHUP` reload.

## 5. Scope guardrails

- Standard library + the three allowed external library categories:
  config parsing (`serde`, `toml`), readline-equivalent, and (bonus only)
  the client/server library. Nothing else.
- No bonus work until every mandatory checkbox above is green.
- No feature flags, no backwards-compat shims — change the code directly.
