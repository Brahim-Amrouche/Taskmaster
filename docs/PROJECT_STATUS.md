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

- `StdinOutputTarget`
- `StdoutOutputTarget`

Convenient aliases are provided:

```rust
type Stdin = OutputTarget<StdinOutputTarget>;
type Stdout = OutputTarget<StdoutOutputTarget>;
```

The `OutputTargetsType` trait defines role-specific behavior through `name()`
and `validate()` methods. Each marker type implements that trait separately.

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

### Current stdin behavior

The stdin validator currently requires the configured path to be an existing
regular file. Therefore:

```toml
stdin = "x"
```

fails unless `x` exists as a file relative to the process working directory.
The path value is interpreted by the operating system; `PathBuf::from("x")`
does not create the file.

The current stdout validator accepts an existing directory and appends
`stdout.log` to it. Its behavior still needs to be completed for nonexistent
output paths and final output-file creation rules.

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
- stdin acceptance of an existing regular file and rejection of missing files
  and directories;
- stdout directory expansion to `stdout.log` and its current acceptance of a
  nonexistent output path;
- configuration-reader behavior before loading, for missing files, valid TOML,
  and invalid TOML.

Run the configuration-parser test suite with:

```text
cargo test -p config-parser
```

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

The current model covers many of these fields, but still needs `stderr`,
`umask`, stronger types, and validation.

## Roadmap

### 1. Configuration contract

Complete the configuration types, defaults, and validation. This is the next
step.

### 2. Single-process lifecycle

Launch one command, configure its environment and output, record its PID, wait
for it, and capture its exit result.

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
