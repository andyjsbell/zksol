use risc0_zkvm::guest::env;
use solana_sbpf::{program::BuiltinProgram, vm::Config};

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
    let mut loader = BuiltinProgram::<SolanaContext>::new_loader(Config {
        enable_symbol_and_section_labels: true,
        reject_broken_elfs: true,
        enable_instruction_tracing: true,
        ..Config::default()
    });

    let input: u32 = env::read();
    env::commit(&input);
}
