# Adding Custom Syscalls in RISC Zero

## Key Components

### 1. Syscall Trait
The host implements the `Syscall` trait:

```rust
pub trait Syscall {
    fn syscall(
        &mut self,
        syscall: &str,
        ctx: &mut SyscallContext,
        to_guest: &mut [u32],
    ) -> Result<()>;
}
```

### 2. SyscallContext
Provides access to VM state and memory:
- Access to registers
- Memory read/write operations
- Cycle count information

### 3. Implementation Pattern

#### Host Side
Implement custom syscall handler:

```rust
// Host side - implement custom syscall handler
struct CustomSyscallHandler;

impl Syscall for CustomSyscallHandler {
    fn syscall(
        &mut self,
        syscall: &str,
        ctx: &mut SyscallContext,
        to_guest: &mut [u32],
    ) -> Result<()> {
        match syscall {
            "custom_operation" => {
                // Read from guest memory/registers
                // Perform host computation
                // Write results back to guest
                Ok(())
            }
            _ => Err(anyhow!("Unknown syscall: {}", syscall))
        }
    }
}

// Register with ExecutorEnv
let env = ExecutorEnv::builder()
    .add_syscall(Box::new(CustomSyscallHandler))
    .build()?;
```

#### Guest Side
Use the `declare_syscall!` macro:

```rust
use risc0_zkvm_platform::syscall::declare_syscall;

declare_syscall!(CUSTOM_OPERATION);

// Call from guest
unsafe {
    CUSTOM_OPERATION.syscall(&args, &mut results);
}
```

## Architecture Notes

In RISC Zero's zkVM:
- The logical RISC-V machine running inside the zkVM is called the "guest"
- The prover running the zkVM is called the "host"
- The guest and host can communicate during execution
- The host cannot modify the guest's execution without invalidating the proof

The syscall mechanism provides the communication bridge between the guest (zkVM) and host environments while maintaining the integrity of the zero-knowledge proof system.

## Built-in Syscalls

RISC Zero includes standard syscalls such as:
- `SYS_ARGC` - Get argument count
- `SYS_ARGV` - Get argument values
- `SYS_CYCLE_COUNT` - Get cycle count
- `SYS_GETENV` - Get environment variables
- `SYS_LOG` - Logging output
- `SYS_PANIC` - Panic handling
- `SYS_RANDOM` - Random number generation
- `SYS_READ` - Read input from host
- `SYS_WRITE` - Write output to host
- `SYS_VERIFY_INTEGRITY` - Verify receipt claim digest

## Printing in Guest

For simple printing to stdout in guest programs:

```rust
use risc0_zkvm::guest::env;

fn main() {
    // Regular println! works in guest code
    println!("Hello from the guest!");
    
    // You can also print variables
    let value = 42;
    println!("The value is: {}", value);
    
    // For debugging, you can use dbg! macro
    let result = dbg!(value * 2);
    
    // env::log() is another option for logging
    env::log("This is a log message");
}
```

Note: Printing in the guest increases the cycle count of your proof, so use it sparingly in production code.