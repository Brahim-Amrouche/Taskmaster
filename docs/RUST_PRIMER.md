# Rust Primer for C/C++ Developers

A short map between what you already know and the Rust equivalents actually
used in this project. Not a textbook — just enough to read and write code in
this repo.

## 1. Ownership, borrowing, lifetimes

- Every value has **one owner**. When the owner goes out of scope, the value
  is dropped (destructor runs). Think C++ `unique_ptr` applied to *every*
  type, enforced at compile time.
- Passing a value **moves** it. The old binding is unusable.
  Equivalent to `std::move` but mandatory, not opt-in.
- Instead of moving, you can **borrow**:
  - `&T` — shared, read-only, many allowed. Like `const T&`.
  - `&mut T` — exclusive, mutable, only one at a time. Like `T&` but the
    compiler enforces no aliasing.
- **Lifetimes** (`'a`) are compile-time labels proving a reference does not
  outlive the data it points to. Most are inferred; write them only when
  the compiler asks.
- No null pointers. Absence is expressed by `Option<T>` (see §3).
- No dangling pointers, no double-free, no use-after-free — these are
  compile errors, not undefined behavior.

### C++ → Rust cheat
| C++ | Rust |
|---|---|
| `T x`             | `let x: T`           |
| `const T&`        | `&T`                 |
| `T&`              | `&mut T`             |
| `std::move(x)`    | `x` (any pass moves) |
| `std::unique_ptr` | `Box<T>`             |
| `std::shared_ptr` | `Arc<T>`             |
| `std::vector`    | `Vec<T>`             |
| `std::string`    | `String` / `&str`    |
| `const char*`    | `&str` (UTF-8) or `&CStr` |

## 2. Mutability

Variables are immutable by default. Add `mut` to opt in:

```rust
let x = 5;        // immutable
let mut y = 5;    // mutable
y += 1;
```

`let` with a new name shadows; it is not reassignment.

## 3. `Option` and `Result` — no exceptions, no `errno`

```rust
enum Option<T> { Some(T), None }
enum Result<T, E> { Ok(T), Err(E) }
```

- Use `Option<T>` where C++ would use a pointer that could be null, or
  `std::optional`.
- Use `Result<T, E>` for fallible operations. There are **no exceptions**;
  errors are values.
- The `?` operator propagates errors: `let x = fallible()?;` returns the
  `Err` early if there is one. Like early-return `if (err) return err;`
  but one character.

```rust
fn parse_octal(s: &str) -> Result<u32, ParseIntError> {
    let n = u32::from_str_radix(s, 8)?;
    Ok(n)
}
```

## 4. Structs, enums, pattern matching

```rust
struct SelfPipe { read_fd: i32, write_fd: i32 }

enum RestartPolicy { Always, Never, Unexpected }

match policy {
    RestartPolicy::Always     => spawn(),
    RestartPolicy::Never      => (),
    RestartPolicy::Unexpected => if !expected { spawn() },
}
```

- Enums are **tagged unions** (like `std::variant`), not just integer
  constants. Variants can carry data.
- `match` is **exhaustive** — the compiler refuses to compile if a variant
  is missing.
- Pattern matching also works in `if let` and `while let`.

## 5. Traits (not inheritance)

Traits are closer to C++20 concepts and interfaces than to virtual-base
classes.

```rust
trait OutputTargetsType {
    fn name() -> &'static str;
    fn validate(path: &Path) -> Result<(), String>;
}

impl OutputTargetsType for StdoutOutputTarget {
    fn name() -> &'static str { "stdout" }
    fn validate(path: &Path) -> Result<(), String> { /* ... */ }
}
```

- Generics are **monomorphized** by default (like C++ templates), zero-cost.
- Dynamic dispatch is opt-in through `dyn Trait` + a vtable pointer.
- There is **no class inheritance**. Compose with traits.
- Common built-in traits: `Debug`, `Display`, `Clone`, `Copy`, `From`,
  `TryFrom`, `Drop`, `PartialEq`, `Eq`, `Hash`. `#[derive(...)]`
  implements them automatically when possible.

## 6. `Drop` = RAII destructor

```rust
impl Drop for SelfPipe {
    fn drop(&mut self) {
        unsafe { close(self.read_fd); close(self.write_fd); }
    }
}
```

Runs deterministically when the value goes out of scope. This is how the
`SelfPipe` closes its file descriptors — same discipline as C++ RAII, with
compile-time ownership guarantees.

## 7. Modules, crates, Cargo

- A **crate** = compilation unit (binary or library). This repo has four.
- A **module** = namespace inside a crate, declared with `mod foo;` which
  looks for `foo.rs` or `foo/mod.rs`. Think C++ namespaces with files.
- `pub` controls visibility. Nothing is public by default.
- `use path::to::Thing;` brings names into scope (like `using`).
- `Cargo.toml` replaces `CMakeLists.txt`. `cargo build`, `cargo test`,
  `cargo check` (type-check without producing a binary — fast feedback loop).

## 8. `unsafe` and FFI (we use this)

`unsafe` disables **four** checks and nothing else:

1. Dereferencing raw pointers.
2. Calling `unsafe` functions (including `extern "C"`).
3. Reading/writing `static mut`.
4. Implementing `unsafe` traits.

Everything else — borrow checking, type checking — still applies. `unsafe`
is a **marker** that you are responsible for the invariants, not a bypass of
the whole language.

FFI declaration (as in our `runtime` crate):

```rust
extern "C" {
    fn pipe2(fds: *mut i32, flags: i32) -> i32;
    fn close(fd: i32) -> i32;
    fn write(fd: i32, buf: *const u8, count: usize) -> isize;
}
```

Call it:

```rust
let mut fds: [i32; 2] = [0; 2];
let rc = unsafe { pipe2(fds.as_mut_ptr(), O_NONBLOCK | O_CLOEXEC) };
if rc != 0 { return Err(std::io::Error::last_os_error()); }
```

Pattern we follow: isolate all `unsafe` + FFI in one small module, wrap it
in a safe RAII type (`SelfPipe`), and keep the rest of the codebase safe.

## 9. Signal handlers and async-signal-safety

The usual Rust runtime (allocator, panic, locks, `println!`) is **not**
async-signal-safe. In a `SIGCHLD` handler you can only:

- call async-signal-safe libc functions (`write`, `_exit`, …),
- read/write `AtomicI32`/`AtomicU8` with `Ordering::Relaxed`.

That is why the handler just writes one byte to the self-pipe and returns.

## 10. Common patterns in this codebase

- **Newtype + marker generics**: `OutputTarget<StdoutOutputTarget>` is a
  single `PathBuf` whose role is encoded in the type. Prevents mixing up a
  stdout path with a stdin path at compile time.
- **`TryFrom<String>`** combined with `#[serde(try_from = "String")]` so
  Serde deserializes a string and then runs validation.
- **Error enums** with `#[derive(Debug)]` and a `Display` impl, propagated
  with `?`.
- **RAII wrappers** around every OS resource (file descriptors, signal
  dispositions). Never leak a `fd`.

## 11. The ten things that will trip you most

1. You can't mutate while borrowed. Scope the borrow smaller, or restructure.
2. `String` (owned, heap) vs `&str` (borrowed slice) — pick based on who
   owns it. API inputs are usually `&str`; return `String`.
3. `Vec<T>` (owned) vs `&[T]` (slice) — same discipline.
4. You can't take two `&mut` into the same container at once. Use `split_at_mut`,
   indices, or restructure.
5. Integer overflow panics in debug, wraps in release. Use the `checked_*`,
   `wrapping_*`, `saturating_*` family when you care.
6. No implicit numeric conversion — use `as` or `TryFrom`.
7. Closures capture by the least-privileged mode they need; use `move` to
   force by-value capture (common when spawning threads).
8. `clone()` is explicit; if you see it, assume it was deliberate.
9. `unwrap()` and `expect()` panic. Fine in tests, suspicious in library code.
10. The compiler is your pair-programmer. Its errors are usually right and
    almost always actionable. Read them in full.

## 12. Suggested one-hour path

1. `rustup` + `cargo new hello`, read `Cargo.toml`.
2. Rust Book chapters 4 (ownership), 6 (enums + match), 9 (errors), 10
   (traits/generics), 15.3 (`Drop`), 19.1 (`unsafe`).
3. Skim `config-parser/src/validators/` here — it exercises traits,
   generics, `TryFrom`, Serde derive, and error propagation in a small
   surface area.
4. Then read `runtime/src/unix/` to see FFI, `unsafe`, and RAII in context.

That is enough to be productive on the mandatory part.
