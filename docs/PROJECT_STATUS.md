# Taskmaster Project Status

## Purpose

Taskmaster is a Rust implementation of a job-control supervisor inspired by
Supervisor. It must start child processes, keep the configured number of
processes alive, restart them according to policy, expose a local control shell,
log important events, and reload its configuration while running.

The bonus features are intentionally outside the current scope.

## Reviewed project state

The repository currently contains a Rust workspace with four crates:

```text
Taskmaster/
├── config-parser/   Configuration structures and TOML reader
├── runtime/         Intended process-supervision library; currently placeholder
├── shell/           Intended control-shell library; currently placeholder
└── taskmaster/      Binary entry point
```

### Implemented

- A Cargo workspace joining all four crates.
- TOML configuration parsing through `serde` and `toml`.
- A generic `ConfigReader` that:
  - receives a configuration path;
  - reads the file into memory;
  - reports file-read errors;
  - refuses to parse before a successful read;
  - deserializes into any compatible Serde type.
- Initial `Config` and `ProgramConfig` structures.
- A sample `Run.toml` configuration.
- A `taskmaster` executable that loads the sample configuration and prints each
  configured command.
- Successful workspace compilation and test execution.

### Not implemented yet

- Process creation and PID tracking.
- Process monitoring and accurate alive/dead state.
- Restart policies and restart limits.
- Graceful stopping and forced killing after a timeout.
- Startup-success timing.
- Logging.
- Interactive control shell.
- `SIGHUP` configuration reload.
- Configuration comparison during reload.
- Runtime and shell functionality.
- Tests for configuration parsing, validation, and process supervision.

## Current quality notes

The workspace currently builds successfully, but it emits two warnings:

- `config-parser/src/config_reader.rs` imports `Config` without using it.
- `taskmaster/src/main.rs` receives `program_name` but does not use it.

These are minor and should be cleaned up when the related files are next
edited. They are not the next functional milestone.

## Recently implemented validation work

The configuration parser now includes a `validators` module:

```text
config-parser/src/validators/
├── mod.rs
└── output_targets.rs
```

The module is exposed from `config-parser/src/lib.rs` and provides validation
types for configuration output targets.

### Generic output targets

`OutputTarget<T>` is a generic structure that stores a `PathBuf`. The generic
parameter identifies the role of the target at compile time.

The current marker types are:

- `StdoutOutputTarget`
- `StderrOutputTarget`

Convenient aliases are provided:

```rust
type Stdout = OutputTarget<StdoutOutputTarget>;
type Stderr = OutputTarget<StderrOutputTarget>;
```

The `OutputTargetsType` trait defines role-specific behavior through `name()`,
`default_target()`, and `validate()` methods. Each marker type implements that
trait separately.

### Serde and `TryFrom`

`OutputTarget<T>` derives `Deserialize` with:

```rust
#[serde(try_from = "String")]
```

This means a TOML string is first deserialized as a `String`, then converted
into the appropriate `OutputTarget<T>` through its `TryFrom<String>`
implementation. The conversion calls `T::validate()` immediately, so invalid
values fail during configuration deserialization.

The conversion is available only for marker types implementing
`OutputTargetsType`:

```rust
impl<T> TryFrom<String> for OutputTarget<T>
where
    T: OutputTargetsType,
```

This prevents unsupported marker types from using the conversion.

### Current output behavior

Both stdout and stderr accept a file path. A directory expands to `stdout.log`
or `stderr.log` respectively, and an empty value selects the corresponding
default file. The selected file is created when the process is started.

### Error propagation

Validation errors are propagated through Serde and the TOML parser into
`ConfigReaderError::ParseError`. The current error chain is:

```text
validator error
  → TryFrom<String>
  → TOML deserialization error
  → ConfigReaderError::ParseError
```

When Rust prints a returned `main` error automatically, it may display the
enum using debug formatting, including escaped characters such as `\\n` and
`\\"`. Printing the error explicitly with `{}` uses the custom `Display`
implementation instead.

This validation work is now part of the project’s implementation record. This
document should continue to be updated whenever a project milestone is
implemented.

## Test coverage

Focused unit tests now cover the configuration functionality implemented so
far. They use the Rust standard library only and create isolated temporary
files under the system temporary directory.

The tests currently verify:

- command rejection for empty input, nonexistent paths, directories, and files
  without an execute bit;
- splitting a valid command into its executable path and arguments;
- `Command::try_from` storing the parsed command;
- stdout and stderr directory expansion to role-specific log filenames and
acceptance of nonexistent output paths;
- working-directory acceptance for existing directories and rejection of empty,
  whitespace-only, missing, and regular-file paths;
- process-count acceptance of positive values and rejection of zero, including
  TOML deserialization coverage;
- umask acceptance of three-digit octal masks and rejection of malformed masks,
  including TOML deserialization coverage;
- restart-policy acceptance of `always`, `never`, and `unexpected`, and
  rejection of unknown policies;
- exit-code acceptance for unique values, rejection of empty and duplicate
  lists, and rejection of TOML values outside the `u8` range;
- stop-signal acceptance for `TERM`, `KILL`, `INT`, `HUP`, and `QUIT`, and
  rejection of unsupported or incorrectly cased signal names;
- configuration-reader behavior before loading, for missing files, valid TOML,
  and invalid TOML.

Run the configuration-parser test suite with:

```text
cargo test -p config-parser
```

## Current implementation milestone

### Configuration validation baseline complete

The sample configuration now deserializes successfully through `ConfigReader`
and `ProgramConfig`. The configuration-parser crate has 38 passing tests and
the workspace passes `cargo check`.

Validated configuration types now cover command/executable paths, working
directories, stdout/stderr targets, process count, restart policy, expected exit
codes, umask, exit signal, and the TOML map shape of environment variables.

### Remaining configuration follow-ups

- Rename the misleading `chmod` key and field to `umask`.
- Validate environment-variable names before process launch.
- Decide and document units and zero-value behavior for startup time, restart
  count, and stop timeout.
- Add full-`Config` integration tests, including invalid nested values.
- Document that command arguments are currently split on whitespace and do not
  support quoted arguments.

### Active milestone: single-process runtime lifecycle

The `runtime` crate is currently unimplemented. Begin with one `ProgramConfig`
and one child process: construct `std::process::Command`, apply the executable
arguments, working directory, environment, stdout/stderr settings, spawn it,
record its PID, wait for its exit, and return a structured exit result.

Do not add restart loops, multiple process instances, signals, reloads, or the
interactive shell in this first runtime milestone. Each requires the reliable
single-process lifecycle as its foundation.

### Runtime architecture decision

The final Unix supervisor will use manual FFI to call the operating system's
process-control APIs. This adds no third-party Rust dependency: the project
will declare only the required Unix functions and constants locally.

The implementation will use `SIGCHLD` for child-exit notifications, a
self-pipe to safely wake the supervisor loop, `waitpid` to reap children, and
`kill` to send configured graceful-stop and forced-stop signals. Begin with
safe standard-library process spawning, then isolate all FFI and `unsafe` code
in one small Unix-specific module before connecting it to supervision logic.

The initial self-pipe FFI checkpoint is complete. The Linux-only `unix` module
declares `pipe2` and `close`, creates a non-blocking close-on-exec pipe, owns
both descriptors through `Drop`, and has a unit test confirming that two valid,
distinct descriptors are created. The next checkpoint is adding read/write
operations so the self-pipe can carry a notification byte.

The self-pipe now carries notification bytes through Unix `write` and `read`
FFI calls. Its tests verify descriptor creation, one-byte notification and
draining, and a second drain returning zero when the pipe is empty. The next
checkpoint is installing a minimal `SIGCHLD` handler that writes this
notification byte without performing normal Rust work in signal context.

The self-pipe now also wraps blocking Unix `poll` and has a test verifying
that a notification makes the read descriptor readable. This is event waiting,
not repeated child-status polling. The next implementation checkpoint remains
the minimal `SIGCHLD` handler, followed by `waitpid` reaping.

The Linux x86_64 `sigaction` FFI structures and installer are now implemented.
The runtime tests save and restore the process-wide `SIGCHLD` disposition while
verifying that handler installation succeeds. The next checkpoint is wiring a
minimal handler to the self-pipe write descriptor; it must only perform a raw
one-byte `write` before returning.

The `SIGCHLD` notification bridge is now implemented and tested end to end.
The handler obtains the configured self-pipe write descriptor from atomic
storage, writes one notification byte, and returns without performing process
management in signal context. The configuration helper clears the stored
descriptor if handler installation fails, and explicit cleanup disables later
writes before the pipe is discarded. The test spawns `/usr/bin/true`, observes
the self-pipe become readable through `poll`, drains it, reaps the test child,
and restores the previous process-wide `SIGCHLD` action.

Non-blocking child reaping is now complete. The Linux `waitpid` FFI is called
with `WNOHANG` in a loop, converting every returned PID and raw wait status to
`ReapedChild` and `ChildTermination` values. The loop retries `EINTR`, ends
normally on `0` or `ECHILD`, and propagates unexpected OS errors. Tests cover
raw normal-exit and signal-termination status decoding plus an end-to-end
child exit, self-pipe wake-up, and manual reaping sequence.

The `SIGCHLD` mask guard is now implemented. It builds a signal set through
`sigemptyset` and `sigaddset`, blocks `SIGCHLD` with `sigprocmask`, retains the
previous thread mask, and restores that exact mask in `Drop`. Tests verify the
mask state and use a thread-directed `SIGCHLD` to confirm delivery is deferred
until the guard is dropped. A process-directed child signal cannot provide the
same deterministic unit test under Rust's multi-threaded test runner because
another unmasked test thread may receive it.

The next implementation stage connects these Unix primitives to Taskmaster's
actual lifecycle: create and configure the self-pipe once, install the handler,
block `SIGCHLD`, spawn and register a `RuntimeProcess`, restore the mask, then
poll, drain, reap, and update the matching process by PID. Configured stop
signals and restart policies come only after this lifecycle is reliable.

### Resume review — 2026-11-06

The runtime source was reviewed against the milestones above. `RuntimeProcess`
currently performs only a basic `Command::new(...).spawn()` and records its PID.
It does not yet apply command arguments, working directory, environment, or
standard-stream configuration; it does not wait for or store a child
termination result; and it is not connected to the self-pipe, `SIGCHLD` handler,
signal-mask guard, or reaping code.

The next implementation task is therefore to build one reliable supervised
lifecycle around the existing Unix primitives: while `SIGCHLD` is blocked,
create the self-pipe and install notifications, spawn and register one process,
restore the previous mask, wait for the pipe, drain it, reap children, and map
the matching PID to its `RuntimeProcess` exit status. Keep this checkpoint to
one configured process and defer restart policies, multiple instances, stop
signals, reloads, and the shell.

Attempting `cargo test --workspace` during this review was blocked by the
development environment: Cargo ran out of disk space while unpacking
`serde_core` in the shared registry. No source or manifest changes were made by
that failed verification attempt.

### Follow-up review — 2026-11-06

The workspace test suite is now verified: all 48 tests pass (39 in
`config-parser` and 9 in `runtime`). A `Supervisor` type has been added with a
`Vec<RuntimeProcess>` field and constructor, but it has no lifecycle behavior
yet. The runtime retains two expected dead-code warnings for the unused
`RuntimeProcess::exit_status` and `Supervisor::processes` fields.

Do not expand the supervisor to multiple processes at this point. First give
the single `RuntimeProcess` the minimal API needed by the supervisor: spawn it
with its parsed command arguments and applicable configuration, expose its PID,
and provide a way for the supervisor to store the matching reaped termination
state. Then add one end-to-end supervisor test that exercises the established
mask → spawn/register → unmask → wait/drain → reap/update sequence.

### Active sub-milestone — RuntimeProcess API

The current task is to turn `RuntimeProcess` into a small callable runtime API
that the future control shell and supervisor can use without duplicating spawn
logic. Its responsibilities are limited to applying one `ProgramConfig`,
starting one child, retaining the PID and lifecycle state, exposing that state,
and accepting a matching reaped termination result. Waiting, `SIGCHLD` handling,
restart decisions, and process-group management remain supervisor
responsibilities.

`RuntimeProcess::start` now applies the parsed executable arguments, working
directory, environment, stdout, and stderr settings before spawning and records
the resulting PID as `Running`. A focused runtime test uses a temporary shell
program to verify all of those settings through its stdout. It reaps only the
child it started. The process test also shares the Unix signal-test lock,
because spawning a child emits process-wide `SIGCHLD` notifications that must
not race tests temporarily installing a handler. The runtime crate now has 10
passing tests.

The next `RuntimeProcess` API work is to expose read-only state and PID access,
reject `start` while already running, and accept a matching reaped termination
result. Only then should `Supervisor` connect it to the self-pipe event loop.

### RuntimeProcess configuration-scope review — 2026-11-07

`RuntimeProcess` now exposes its state, PID, running status, PID matching, and
exit recording. Its launch behavior applies `command`, arguments, `chdir`,
environment variables, stdout, and stderr. The remaining required setting
that affects child construction is `umask`; it must be applied while creating
the child process. Process count, autostart, restart
policy, expected exit codes, startup timing, retry count, stop signal, and
stop timeout are supervisor policy and must not be implemented as hidden loops
inside `RuntimeProcess`.

Before supervisor integration, refine process state transitions: reject only a
second start while `Running`; permit a new start after an exit for a supervisor
approved restart; include the reaped PID in exit recording and reject a
mismatched or non-running process; and introduce `Starting` and `Stopping`
states when timing and graceful-stop behavior are added.

### RuntimeProcess lifecycle API complete — 2026-11-07

The current `RuntimeProcess` API now implements the reviewed transition rules:
it rejects a second start while `Running`, permits restart after `Exited`, and
records an exit only when a matching running PID is provided. Tests verify the
launch configuration, state/PID queries, duplicate-start rejection, mismatched
PID rejection, exit transition, and restart after a recorded exit. The runtime
suite has 10 passing tests.

The next runtime milestone is now safe to begin: let `Supervisor` own the
single self-pipe lifecycle and use `reap_exited_children` to find a matching
`RuntimeProcess`, then call its PID-checked exit-recording API. Do not add
restart policy decisions yet; first prove one child can move from `Running` to
`Exited` via the actual SIGCHLD notification path.

### Deferred supervisor work — process-construction focus

Supervisor integration is intentionally deferred. The active work is instead
to complete the child-process construction contract. Stdout and stderr are now
represented independently and are both applied by `RuntimeProcess`; decide
their final file-open behavior, rename `chmod` to `umask`, and apply the
configured umask in the child immediately before exec. `RuntimeProcess` should
then build a child with command arguments, working directory, environment,
stdout, stderr, and umask. Process count,
autostart, restart policy, expected exit codes, startup time, restart count,
stop signal, and stop timeout remain stored configuration only until the
supervisor milestone resumes.

### Umask launch handling complete — 2026-11-07

The configuration key and field are now named `umask`. When it is present,
`RuntimeProcess` installs a minimal Unix `pre_exec` hook that applies the
validated mask in the forked child before the configured command executes. The
parent Taskmaster process is therefore unaffected. The runtime launch test
configures `027`, has the child create a file, and verifies its mode is `0640`.

The next process-construction task is finalizing output-target behavior. Choose
and test append or truncate semantics for redirected files, then add an
explicit discard representation that maps stdout or stderr to `Stdio::null()`.

### Output-target behavior complete — 2026-11-07

`stdout` and `stderr` are optional configuration fields. When either field is
omitted, `RuntimeProcess` redirects that stream to `Stdio::null()`. When a
field is supplied, its file is created as needed and truncated on each child
start. Tests verify existing-file truncation and successful child execution
with both streams discarded. The workspace currently passes 39
configuration-parser tests and 11 runtime tests.

### Next configuration task — environment variables

The remaining launch-time configuration follow-up is validating environment
variable names while deserializing `ProgramConfig`. Reject invalid names before
process creation, using the POSIX-style rule that a name begins with an ASCII
letter or underscore and then contains only ASCII letters, digits, or
underscores. Keep values as arbitrary strings. Once this validation is covered
by parser tests, the child-construction contract will be complete; the
remaining assignment settings are supervisor policy.

### Environment validation complete — 2026-11-07

`ProgramConfig.env` now uses the `EnvironmentVariables` validator instead of a
raw map. TOML deserialization rejects names that are empty, begin with anything
other than an ASCII letter or underscore, or contain characters outside ASCII
letters, digits, and underscores. Values remain arbitrary strings. The runtime
passes the validated map to `Command::envs`. The workspace passes 45
configuration-parser tests and 11 runtime tests.


## Configuration fields required by the assignment

Each supervised program must eventually describe:

- command to launch;
- number of processes;
- whether to start automatically;
- restart policy: always, never, or unexpected exits only;
- expected exit codes;
- startup-success duration;
- maximum restart attempts;
- graceful-stop signal;
- time before forced termination;
- stdout destination;
- stderr destination;
- environment variables;
- working directory;
- umask.

The current model covers many of these fields, but still needs `umask` to be
applied at process launch, stronger types, and validation.

## Roadmap

### 1. Configuration contract

The initial configuration contract and field-level validation are complete.
Finish the documented follow-ups while adding runtime support for the affected
fields.

### 2. Single-process lifecycle

Launch one command, configure its environment and output, record its PID, wait
for it, and capture its exit result. This is the active major milestone.

### 3. Supervisor runtime

Manage multiple instances, maintain the desired process count, detect exits,
apply restart policies, enforce restart limits, and implement graceful stop
followed by forced kill.

### 4. Logging

Log starts, stops, restarts, exits, failures, restart exhaustion, reloads, and
shutdown events to a local file.

### 5. Control shell

Implement `status`, `start`, `stop`, `restart`, `reload`, and `quit`/`shutdown`.

### 6. Reloading

Handle `SIGHUP`, validate the new configuration before applying it, preserve
unchanged processes, and add, remove, or update programs as necessary.

### 7. Integration testing

Test long-running commands, immediate failures, unexpected exits, output-heavy
commands, multiple process counts, invalid configurations, manual kills, and
configuration changes.

## Scope decision

The client/server architecture, privilege de-escalation, advanced reporting,
and console attachment are bonus features and should not be implemented until
all mandatory features work reliably.
