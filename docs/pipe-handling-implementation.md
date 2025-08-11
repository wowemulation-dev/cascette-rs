# CLI Pipe Handling Implementation Guide

## Problem Statement

When piping the CLI output to commands like `jq`, `head`, or `grep`, the program may panic with a "Broken pipe" error when the receiving command closes the pipe early. This is especially common with commands like:

```bash
ngdp --format json products list | head -5
ngdp --format json inspect encoding wow | jq '.header'
```

## Root Cause

When a pipe reader (like `head`) closes the pipe after reading enough data, the writer (our CLI) receives a SIGPIPE signal on Unix systems or an IO error on Windows. By default, Rust programs panic when encountering a broken pipe error during stdout writes.

## Solution Approach

### 1. Detect Piped Output

```rust
use atty::Stream;

fn is_stdout_piped() -> bool {
    !atty::is(Stream::Stdout)
}
```

### 2. Handle SIGPIPE on Unix

```rust
#[cfg(unix)]
fn ignore_sigpipe() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
}
```

### 3. Wrap Output Writes

```rust
use std::io::{self, Write};

fn write_output(data: &str) -> io::Result<()> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    
    match handle.write_all(data.as_bytes()) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
            // Silently exit on broken pipe
            std::process::exit(0);
        }
        Err(e) => Err(e),
    }
}
```

### 4. Custom Panic Hook

```rust
use std::panic;

fn setup_panic_handler() {
    let default_hook = panic::take_hook();
    
    panic::set_hook(Box::new(move |panic_info| {
        // Check if this is a broken pipe panic
        if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            if s.contains("Broken pipe") {
                // Exit silently for broken pipes
                std::process::exit(0);
            }
        }
        
        // Otherwise use the default handler
        default_hook(panic_info);
    }));
}
```

### 5. Main Function Integration

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up signal handling
    #[cfg(unix)]
    ignore_sigpipe();
    
    // Set up panic handler
    setup_panic_handler();
    
    // Parse CLI arguments
    let cli = Cli::parse();
    
    // Configure tracing with pipe awareness
    let is_piped = is_stdout_piped();
    configure_tracing(cli.log_level, is_piped);
    
    // Rest of the application...
}
```

### 6. Tracing Configuration

```rust
fn configure_tracing(level: Level, is_piped: bool) {
    let builder = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false);
    
    if is_piped {
        // Disable ANSI colors and progress indicators for pipes
        builder
            .with_ansi(false)
            .without_time()
            .init();
    } else {
        // Normal TTY output with colors
        builder.init();
    }
}
```

## Testing Strategy

### Manual Testing

```bash
# Test with head (closes pipe early)
cargo run -- --format json products list | head -5

# Test with jq (may close pipe after filtering)
cargo run -- --format json inspect encoding wow | jq '.header'

# Test with grep (selective reading)
cargo run -- products list | grep wow

# Test pipe chains
cargo run -- --format json products list | jq '.[] | select(.code | startswith("wow"))' | head -3

# Test with tee (should not break)
cargo run -- products list | tee output.txt | head -5
```

### Automated Testing

```rust
#[test]
fn test_broken_pipe_handling() {
    use std::process::{Command, Stdio};
    use std::io::Write;
    
    let mut child = Command::new("target/debug/ngdp")
        .args(&["--format", "json", "products", "list"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn child");
    
    // Close the pipe early
    drop(child.stdout.take());
    
    // Process should exit cleanly
    let status = child.wait().expect("Failed to wait for child");
    assert!(status.success() || status.code() == Some(141)); // 141 = 128 + SIGPIPE
}
```

## Implementation Priority

1. **Phase 1**: Basic broken pipe handling
   - Add SIGPIPE handler
   - Wrap stdout writes
   - Test with common pipe commands

2. **Phase 2**: Enhanced UX
   - Detect piped output
   - Disable progress bars when piped
   - Add streaming JSON support

3. **Phase 3**: Advanced features
   - NDJSON output format
   - Configurable buffering
   - Resume support for interrupted operations

## Dependencies

```toml
[dependencies]
atty = "0.2"  # TTY detection
libc = "0.2"  # For signal handling (Unix)

[dev-dependencies]
assert_cmd = "2.0"  # For integration testing
predicates = "3.0"  # For test assertions
```

## Platform Notes

### Linux/macOS
- SIGPIPE handling is critical
- Exit code 141 (128 + SIGPIPE) is expected

### Windows
- No SIGPIPE, but broken pipe errors still occur
- Different error handling required
- Exit code 0 or 1 depending on error type

## References

- [Rust std::io::ErrorKind::BrokenPipe](https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.BrokenPipe)
- [Unix signal handling](https://www.gnu.org/software/libc/manual/html_node/Signal-Handling.html)
- [Pipe behavior in shells](https://www.gnu.org/software/bash/manual/html_node/Pipelines.html)