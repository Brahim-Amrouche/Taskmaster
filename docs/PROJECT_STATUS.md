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

