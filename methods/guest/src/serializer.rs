use risc0_zkvm::guest::env;
use solana_sbpf::{
    aligned_memory::{AlignedMemory, Pod},
    ebpf::{HOST_ALIGN, MM_INPUT_START},
    memory_region::MemoryRegion,
};

use crate::runtime::{Account, Pubkey};

/// Serializer for converting Solana account data into SBPF VM memory format.
/// Handles memory layout, alignment, and region management for VM input.
pub struct Serializer {
    buffer: AlignedMemory<HOST_ALIGN>,
    regions: Vec<MemoryRegion>,
    vaddr: Address,
    region_start: usize,
}

// Solana-specific constants for memory alignment and account handling
pub const BPF_ALIGN_OF_U128: usize = 8;
pub const NON_DUP_MARKER: u8 = u8::MAX;
pub const MAX_PERMITTED_DATA_INCREASE: usize = 1_024 * 10; // 10KB max growth
pub type Address = u64;

/// Represents a serialized account in VM memory with address pointers.
#[allow(dead_code)]
pub struct VmSerializedAccount {
    public_key_addr: Address,
    owner_key_addr: Address,
    lamports_addr: Address,
    data_addr: Address,
    pub original_data_len: usize,
}

impl Serializer {
    /// Creates a new serializer with specified buffer size and starting virtual address.
    pub fn new(size: usize, start_addr: Address) -> Self {
        Serializer {
            buffer: AlignedMemory::with_capacity(size),
            regions: Vec::new(),
            vaddr: start_addr,
            region_start: 0,
        }
    }

    fn fill(&mut self, num: usize, value: u8) -> Result<(), &str> {
        self.buffer.fill_write(num, value).map_err(|_| "todo")
    }

    /// Writes a POD (Plain Old Data) value to the buffer and returns its virtual address.
    fn write<T: Pod>(&mut self, value: T) -> Address {
        self.debug_assert_alignment::<T>();
        let vaddr = self
            .vaddr
            .saturating_add(self.buffer.len() as u64)
            .saturating_sub(self.region_start as u64);
        unsafe {
            self.buffer.write_unchecked(value);
        }

        vaddr
    }

    fn write_all(&mut self, value: &[u8]) -> Address {
        let vaddr = self
            .vaddr
            .saturating_add(self.buffer.len() as u64)
            .saturating_sub(self.region_start as u64);
        unsafe {
            self.buffer.write_all_unchecked(value);
        }

        vaddr
    }

    fn push_region(&mut self) {
        let range = self.region_start..self.buffer.len();

        let memory_region = MemoryRegion::new_writable(
            self.buffer
                .as_slice_mut()
                .get_mut(range.clone())
                .expect("a mutable slice"),
            self.vaddr,
        );

        self.regions.push(memory_region);
        self.region_start = range.end;
        self.vaddr += range.len() as Address;
    }

    fn finish(mut self) -> (AlignedMemory<HOST_ALIGN>, Vec<MemoryRegion>) {
        self.push_region();
        (self.buffer, self.regions)
    }

    /// Writes account data with padding for potential growth during execution.
    fn write_account(&mut self, account: &mut Account) -> Address {
        let vm_data_addr = self.vaddr.saturating_add(self.buffer.len() as u64);
        self.write_all(&account.data);
        let align_offset = (self.buffer.len() as *const u8).align_offset(BPF_ALIGN_OF_U128);
        self.fill(MAX_PERMITTED_DATA_INCREASE + align_offset, 0)
            .expect("invalid argument");

        vm_data_addr
    }

    /// Serializes accounts and instruction data in Solana's input format.
    /// Returns memory buffer, memory regions for VM mapping, and account metadata.
    pub fn serialize_parameters(
        accounts: Vec<Account>,
        instruction_data: &[u8],
        program_id: Pubkey,
    ) -> (
        AlignedMemory<HOST_ALIGN>,
        Vec<MemoryRegion>,
        Vec<VmSerializedAccount>,
    ) {
        env::log(&format!("number of accounts: {}", accounts.len()));

        // Calculate total buffer size needed for serialization

        let mut size = size_of::<u64>();
        for account in &accounts {
            let data_len = account.data.len();
            size += 1 // dup
            + size_of::<u8>() // is_signer
            + size_of::<u8>() // is_writable
            + size_of::<u8>() // executable
            + size_of::<u32>() // original_data_len
            + size_of::<Pubkey>()  // key
            + size_of::<Pubkey>() // owner
            + size_of::<u64>()  // lamports
            + size_of::<u64>()  // data len
            + size_of::<u64>() // rent epoch
            + data_len
                + MAX_PERMITTED_DATA_INCREASE
                + (size as *const u8).align_offset(BPF_ALIGN_OF_U128);
        }

        size += size_of::<u64>(); // data len
        size += instruction_data.len();
        size += size_of::<Pubkey>(); // program id;

        let mut serialized_accounts = Vec::new();
        let mut s = Self::new(size, MM_INPUT_START);

        // Serialize accounts in Solana's expected format
        s.write((accounts.len() as u64).to_le());
        for mut account in accounts {
            s.write::<u8>(NON_DUP_MARKER);
            s.write::<u8>(account.is_signer as u8);
            s.write::<u8>(account.is_writable as u8);
            s.write::<u8>(account.executable as u8);
            s.write_all(&[0u8, 0, 0, 0]);
            let public_key_addr = s.write_all(account.pubkey.as_ref());
            let owner_key_addr = s.write_all(account.owner.as_ref());
            let lamports_addr = s.write::<u64>(account.lamports.to_le());
            s.write::<u64>((account.data.len() as u64).to_le());
            let data_addr = s.write_account(&mut account);
            // Rent epoch
            s.write::<u64>(account.rent_epoch.to_le());

            serialized_accounts.push(VmSerializedAccount {
                public_key_addr,
                owner_key_addr,
                lamports_addr,
                data_addr,
                original_data_len: account.data.len(),
            });
        }

        s.write::<u64>((instruction_data.len() as u64).to_le());
        s.write_all(instruction_data);
        s.write_all(program_id.as_ref());
        let (memory, regions) = s.finish();

        (memory, regions, serialized_accounts)
    }

    fn debug_assert_alignment<T>(&self) {
        debug_assert!(
            self.buffer
                .as_slice()
                .as_ptr_range()
                .end
                .align_offset(align_of::<T>())
                == 0
        );
    }
}
