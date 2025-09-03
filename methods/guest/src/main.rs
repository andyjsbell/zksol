use crate::runtime::Pubkey;
use crate::serializer::Serializer;
use risc0_zkvm::guest::env;
use solana_sbpf::aligned_memory::AlignedMemory;
use solana_sbpf::declare_builtin_function;
use solana_sbpf::elf::Executable;
use solana_sbpf::error::StableResult;
use solana_sbpf::memory_region::{MemoryMapping, MemoryRegion};
use solana_sbpf::vm::EbpfVm;
use solana_sbpf::{program::BuiltinProgram, vm::Config};
use std::slice;
use std::sync::Arc;
mod runtime;
mod serializer;

extern crate alloc;

#[derive(Default)]
pub struct SolanaContext {
    pub compute_units_remaining: u64,
    pub compute_units_consumed: u64, // Track total consumption for monitoring
}

impl SolanaContext {
    pub fn consume_compute_units(&mut self, units: u64) {
        let consumed = units.min(self.compute_units_remaining);
        self.compute_units_remaining = self.compute_units_remaining.saturating_sub(units);
        self.compute_units_consumed += consumed;
    }
}

impl SolanaContext {
    pub fn consume_gas(&mut self, units: u64) {
        self.consume_compute_units(units);
    }
}

impl solana_sbpf::vm::ContextObject for SolanaContext {
    fn trace(&mut self, _state: [u64; 12]) {
        // Optional: implement tracing for debugging
    }

    fn consume(&mut self, amount: u64) {
        self.consume_compute_units(amount);
    }

    fn get_remaining(&self) -> u64 {
        self.compute_units_remaining
    }
}

declare_builtin_function!(
    SyscallLog,
    fn rust(
        context: &mut SolanaContext,
        addr: u64,
        len: u64,
        _arg3: u64,
        _arg4: u64,
        _arg5: u64,
        memory_mapping: &mut MemoryMapping,
    ) -> Result<u64, Box<dyn core::error::Error + Send + Sync>> {
        context.consume_gas(1);

        // Map the memory region and get the host address
        let host_addr = memory_mapping
            .map(solana_sbpf::memory_region::AccessType::Load, addr, len)
            .map_err(|e| format!("Memory mapping failed: {:?}", e))
            .unwrap();

        // Create a slice from the mapped memory
        let msg_slice = unsafe { slice::from_raw_parts(host_addr as *const u8, len as usize) };

        // Convert bytes to UTF-8 string
        let message = str::from_utf8(msg_slice).map_err(|_| "Invalid UTF-8 in log message")?;

        env::log(message);

        Ok(0)
    }
);

declare_builtin_function!(
    SyscallAbort,
    fn rust(
        _context: &mut SolanaContext,
        arg1: u64,
        arg2: u64,
        arg3: u64,
        arg4: u64,
        arg5: u64,
        _memory_mapping: &mut MemoryMapping,
    ) -> Result<u64, Box<dyn core::error::Error + Send + Sync>> {
        env::log(&format!(
            "Abort args: {:x} {:x} {:x} {:x} {:x}",
            arg1, arg2, arg3, arg4, arg5
        ));
        Err("Program aborted".into())
    }
);

declare_builtin_function!(
    SyscallMemcpy,
    fn rust(
        context: &mut SolanaContext,
        dst_addr: u64,
        src_addr: u64,
        n: u64,
        _arg4: u64,
        _arg5: u64,
        memory_mapping: &mut MemoryMapping,
    ) -> Result<u64, Box<dyn core::error::Error + Send + Sync>> {
        context.consume_gas(n);

        let dst_ptr =
            match memory_mapping.map(solana_sbpf::memory_region::AccessType::Store, dst_addr, n) {
                StableResult::Ok(ptr) => ptr,
                StableResult::Err(e) => {
                    return Err(format!("Destination memory mapping failed: {:?}", e).into())
                }
            };
        let src_ptr =
            match memory_mapping.map(solana_sbpf::memory_region::AccessType::Load, src_addr, n) {
                StableResult::Ok(ptr) => ptr,
                StableResult::Err(e) => {
                    return Err(format!("Source memory mapping failed: {:?}", e).into())
                }
            };

        unsafe {
            core::ptr::copy_nonoverlapping(src_ptr as *const u8, dst_ptr as *mut u8, n as usize);
        }

        Ok(0)
    }
);

declare_builtin_function!(
    SyscallMemmove,
    fn rust(
        context: &mut SolanaContext,
        dst_addr: u64,
        src_addr: u64,
        n: u64,
        _arg4: u64,
        _arg5: u64,
        memory_mapping: &mut MemoryMapping,
    ) -> Result<u64, Box<dyn core::error::Error + Send + Sync>> {
        context.consume_gas(n);
        env::log(&format!(
            "sol_memmove_: dst=0x{:x}, src=0x{:x}, len={}",
            dst_addr, src_addr, n
        ));

        let dst_ptr =
            match memory_mapping.map(solana_sbpf::memory_region::AccessType::Store, dst_addr, n) {
                StableResult::Ok(ptr) => ptr,
                StableResult::Err(e) => {
                    return Err(format!("Destination memory mapping failed: {:?}", e).into())
                }
            };
        let src_ptr =
            match memory_mapping.map(solana_sbpf::memory_region::AccessType::Load, src_addr, n) {
                StableResult::Ok(ptr) => ptr,
                StableResult::Err(e) => {
                    return Err(format!("Source memory mapping failed: {:?}", e).into())
                }
            };

        unsafe {
            core::ptr::copy(src_ptr as *const u8, dst_ptr as *mut u8, n as usize);
        }

        Ok(0)
    }
);

declare_builtin_function!(
    SyscallMemset,
    fn rust(
        context: &mut SolanaContext,
        addr: u64,
        c: u64,
        n: u64,
        _arg4: u64,
        _arg5: u64,
        memory_mapping: &mut MemoryMapping,
    ) -> Result<u64, Box<dyn core::error::Error + Send + Sync>> {
        context.consume_gas(n);
        env::log(&format!(
            "sol_memset_: addr=0x{:x}, val={}, len={}",
            addr, c, n
        ));

        let ptr = match memory_mapping.map(solana_sbpf::memory_region::AccessType::Store, addr, n) {
            StableResult::Ok(ptr) => ptr,
            StableResult::Err(e) => return Err(format!("Memory mapping failed: {:?}", e).into()),
        };

        unsafe {
            core::ptr::write_bytes(ptr as *mut u8, c as u8, n as usize);
        }

        Ok(0)
    }
);

declare_builtin_function!(
    SyscallMemcmp,
    fn rust(
        context: &mut SolanaContext,
        addr1: u64,
        addr2: u64,
        n: u64,
        _arg4: u64,
        _arg5: u64,
        memory_mapping: &mut MemoryMapping,
    ) -> Result<u64, Box<dyn core::error::Error + Send + Sync>> {
        context.consume_gas(n);
        env::log(&format!(
            "sol_memcmp_: addr1=0x{:x}, addr2=0x{:x}, len={}",
            addr1, addr2, n
        ));

        let ptr1 = match memory_mapping.map(solana_sbpf::memory_region::AccessType::Load, addr1, n)
        {
            StableResult::Ok(ptr) => ptr,
            StableResult::Err(e) => {
                return Err(format!("First memory mapping failed: {:?}", e).into())
            }
        };
        let ptr2 = match memory_mapping.map(solana_sbpf::memory_region::AccessType::Load, addr2, n)
        {
            StableResult::Ok(ptr) => ptr,
            StableResult::Err(e) => {
                return Err(format!("Second memory mapping failed: {:?}", e).into())
            }
        };

        let slice1 = unsafe { slice::from_raw_parts(ptr1 as *const u8, n as usize) };
        let slice2 = unsafe { slice::from_raw_parts(ptr2 as *const u8, n as usize) };

        let result = match slice1.cmp(slice2) {
            core::cmp::Ordering::Less => -1i32,
            core::cmp::Ordering::Equal => 0i32,
            core::cmp::Ordering::Greater => 1i32,
        };

        Ok(result as u64)
    }
);

fn register_syscalls(
    loader: &mut BuiltinProgram<SolanaContext>,
) -> Result<(), Box<dyn core::error::Error>> {
    loader.register_function("sol_log_", SyscallLog::vm)?;
    loader.register_function("abort", SyscallAbort::vm)?;
    loader.register_function("sol_panic_", SyscallAbort::vm)?;
    loader.register_function("sol_memcpy_", SyscallMemcpy::vm)?;
    loader.register_function("sol_memmove_", SyscallMemmove::vm)?;
    loader.register_function("sol_memset_", SyscallMemset::vm)?;
    loader.register_function("sol_memcmp_", SyscallMemcmp::vm)?;
    Ok(())
}

fn main() {
    let bytecode: Vec<u8> = env::read();

    let mut loader = BuiltinProgram::<SolanaContext>::new_loader(Config {
        enable_symbol_and_section_labels: true,
        reject_broken_elfs: true,
        enable_instruction_tracing: true,
        ..Config::default()
    });

    register_syscalls(&mut loader).expect("Failed to register syscalls");

    let executable = match Executable::from_elf(&bytecode, Arc::new(loader)) {
        Ok(exec) => {
            env::log(&format!(
                "Detected SBPF Version: {:?}",
                exec.get_sbpf_version()
            ));
            exec
        }
        Err(e) => {
            panic!("Failed to create executable: {:?}", e);
        }
    };
    let sbpf_version = executable.get_sbpf_version();
    let config = executable.get_config();
    let stack_size = config.stack_size();

    let mut stack = AlignedMemory::<{ solana_sbpf::ebpf::HOST_ALIGN }>::zero_filled(stack_size);
    let stack_len = stack.len();

    let heap_size = 32 * 1024;
    let mut heap = AlignedMemory::<{ solana_sbpf::ebpf::HOST_ALIGN }>::zero_filled(heap_size);

    let program_id = Pubkey::try_from("zkRXxvKMqQYgPRAkBHwYKCvnF8YjVtXW1BK4VCXpkeo".to_string())
        .expect("valid bs58");

    let (_, parameter_regions, _) = Serializer::serialize_parameters(vec![], &[], program_id);

    let regions: Vec<MemoryRegion> = vec![
        executable.get_ro_region(),
        MemoryRegion::new_writable_gapped(
            stack.as_slice_mut(),
            solana_sbpf::ebpf::MM_STACK_START,
            if !sbpf_version.dynamic_stack_frames() && config.enable_stack_frame_gaps {
                config.stack_frame_size as u64
            } else {
                0
            },
        ),
        MemoryRegion::new_writable(heap.as_slice_mut(), solana_sbpf::ebpf::MM_HEAP_START),
    ]
    .into_iter()
    .chain(parameter_regions)
    .collect();

    let memory_mapping = match MemoryMapping::new(regions, config, sbpf_version) {
        Ok(m) => m,
        Err(e) => panic!("Failed to create memory regions: {:?}", e),
    };

    let mut context = SolanaContext {
        compute_units_remaining: 200_000, // Solana default compute budget
        compute_units_consumed: 0,
    };
    let mut vm = EbpfVm::new(
        executable.get_loader().clone(),
        sbpf_version,
        &mut context,
        memory_mapping,
        stack_len,
    );

    let (instruction_count, result) = vm.execute_program(&executable, true);
    env::log(&format!("Instruction Count: {}", instruction_count));
    env::log(&format!("Result: {:?}", result));
    env::commit(&result.is_ok());
}
