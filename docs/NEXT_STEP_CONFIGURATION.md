# Next Step: Complete and Validate the Configuration Model

## Objective

Make the configuration layer a reliable contract for the runtime. When this
step is complete, a valid configuration should deserialize into clear Rust
types, and an invalid configuration should fail before any child process is
started.

The runtime should not need to interpret arbitrary strings such as `"always"`
or `"TERM"` throughout its process-management code.

## Work sequence

### 1. Compare the model with the assignment

Review `config-parser/src/config.rs` and ensure the model represents every
mandatory setting:

- command;
- process count;
- autostart;
- restart policy;
- expected exit codes;
- startup-success time;
- restart limit;
- stop signal;
- stop timeout;
- stdout;
- stderr;
- environment variables;
- working directory;
- umask.

Add the missing `stderr` and `umask` fields. Keep the names consistent with
the chosen `Run.toml` format.

### 2. Replace policy strings with enums

Introduce an enum for restart behavior, for example:

```rust
enum RestartPolicy {
    Always,
    Never,
    Unexpected,
}
```

The exact names are your choice, but the deserialized TOML values must be
unambiguous. The runtime can then match on the enum instead of comparing
strings.

Use a similarly clear representation for the stop signal. If the first
implementation supports a limited set of Unix signals, reject unsupported
names during validation rather than silently accepting them.

### 3. Choose representations for optional paths

Decide how the configuration expresses output behavior:

- a file path redirects output to that file;
- an explicit discard value redirects to `/dev/null`;
- an omitted value can use a documented default.

Use `Option<PathBuf>` or an explicit output enum if that makes the behavior
clearer. Apply the same rule independently to stdout and stderr.

### 4. Represent environment variables clearly

Prefer a key/value representation such as a map rather than an unstructured
list of strings. The configuration should make it obvious which part is the
variable name and which part is the value.

For example, choose and document one format:

```toml
[program.worker.env]
MODE = "production"
ANSWER = "42"
```

Do not support multiple competing formats at this stage.

### 5. Add validation

Create a validation method or validation function that checks at least:

- command is not empty;
- process count is greater than zero;
- startup duration, stop timeout, and restart limit are valid for the chosen
  semantics;
- restart policy is valid;
- stop signal is supported;
- expected exit-code values are valid;
- paths and output settings are internally consistent;
- environment variable names are not empty.

Validation should return a useful error identifying the program and field that
failed. Do not start any process until all programs have passed validation.

### 6. Update the sample configuration

Update `Run.toml` so it demonstrates the final field names and valid values.
Use a small command that is available in the project VM, and ensure the sample
does not contain accidental whitespace in policy values.

### 7. Add parser tests

Add focused tests for:

1. a valid configuration;
2. a missing required field;
3. an invalid restart policy;
4. an invalid stop signal;
5. a zero process count;
6. an invalid environment entry;
7. parsing attempted before `read()`;
8. a missing configuration file.

These tests should test the configuration layer only. Process-launch tests
belong to the next milestone.

## Completion checklist

- [ ] Every mandatory assignment field has a model representation.
- [ ] Restart policy is represented by a validated type.
- [ ] Stop signal is represented by a validated type.
- [ ] Stdout and stderr behavior is defined.
- [ ] Environment variables have a clear representation.
- [ ] Invalid configurations produce useful errors.
- [ ] `Run.toml` demonstrates the accepted format.
- [ ] Parser and validation tests pass.
- [ ] No child process is launched by the configuration layer.

## What comes immediately after this step

Once the checklist is complete, begin the single-process lifecycle in
`runtime`. Start with one command and no restart loop: launch it, configure its
working directory/environment/output, record its PID, wait for termination,
and return a structured exit result. Only after that works should restart
policies and multi-process supervision be added.

