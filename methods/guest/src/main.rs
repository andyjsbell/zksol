use crate::{runtime::Pubkey, serializer::Serializer};
use risc0_zkvm::guest::env;
use solana_sbpf::{
    aligned_memory::AlignedMemory,
    elf::Executable,
    memory_region::{MemoryMapping, MemoryRegion},
    program::BuiltinProgram,
    vm::{Config, EbpfVm},
};
use std::sync::Arc;
mod runtime;
mod serializer;
mod syscalls;

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

fn main() {
    let bytecode: Vec<u8> = env::read();

    let mut loader = BuiltinProgram::<SolanaContext>::new_loader(Config {
        enable_symbol_and_section_labels: true,
        reject_broken_elfs: true,
        enable_instruction_tracing: true,
        ..Config::default()
    });

    syscalls::register_syscalls(&mut loader).expect("Failed to register syscalls");

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
