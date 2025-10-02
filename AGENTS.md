# Agent Guidelines for prime-ble-firmware

## Build/Lint/Test Commands

### Building
- Full production build: `cargo xtask build-fw-image`
- Debug build: `cargo xtask build-fw-debug-image`
- Minimal build: `cargo xtask build-minimal-image`
- Quick build: `just build`

### Testing
- Run all tests: `cargo test`
- Run protocol tests: `just test-encoding` or `cargo test --release --package host-protocol --lib`
- Run tests with output: `cargo test -- --nocapture`
- Run single test: `cargo test test_name`

### Linting & Formatting
- Lint: `cargo clippy`
- Format: `cargo fmt`
- Check formatting: `cargo fmt --check`

## Code Style Guidelines

### Formatting
- Max line width: 140 characters
- Use rustfmt for consistent formatting
- 4-space indentation (standard Rust)

### Naming Conventions
- Types/Enums/Structs: PascalCase (`Bluetooth`, `TxPower`, `Server`)
- Functions/Methods/Variables: snake_case (`run_bluetooth`, `bt_state`, `device_id`)
- Constants: SCREAMING_SNAKE_CASE (`BT_MAX_NUM_PKT`, `MAX_MSG_SIZE`)
- Modules: snake_case (`comms`, `server`, `nus`)

### Imports
- Group imports: std, external crates, then local modules
- Use explicit imports over glob imports
- Example:
```rust
use core::cell::RefCell;
use embassy_nrf::gpio::{Level, Output};
use host_protocol::Message;
```

### Types & Generics
- Use strong typing with generics where appropriate
- Prefer `&[u8]` over `Vec<u8>` for read-only data
- Use heapless collections for no_std compatibility (`heapless::Vec`, `heapless::String`)
- Derive common traits: `#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]`

### Error Handling
- Use `Result<T, E>` for fallible operations
- Use `unwrap!()` macro for cases that should never fail in correct operation
- Handle errors appropriately in embedded context
- Use `expect("descriptive message")` for critical failures

### Documentation
- Module docs: `//! Description of module purpose`
- Item docs: `/// Description of function/struct/enum`
- Include parameter and return descriptions
- Document safety requirements for unsafe code

### Code Structure
- Keep functions focused and reasonably sized
- Use meaningful variable names
- Group related functionality in modules
- Use feature flags for conditional compilation (`#[cfg(feature = "debug")]`)

### Embedded Considerations
- Minimize heap allocations in hot paths
- Use static variables with `Mutex` for shared state
- Prefer compile-time evaluation where possible
- Be mindful of stack usage in interrupt contexts