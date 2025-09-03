// Copyright (c) 2025 Andy Bell <andyjsbell@gmail.com>
// SPDX-License-Identifier: MIT

use crate::SolanaContext;
use risc0_zkvm::guest::env;
use solana_sbpf::{
    declare_builtin_function, error::StableResult, memory_region::MemoryMapping,
    program::BuiltinProgram,
};
use std::slice;

// Implements Solana's sol_log_ syscall for printing messages.
// Maps guest memory to host memory and outputs the message to the zkVM log.
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

// Implements program abort syscall.
// Logs abort arguments and terminates execution with an error.
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

// Implements sol_memcpy_ syscall for memory copying.
// Safely maps source and destination memory regions before copying.
// Consumes compute units proportional to bytes copied.
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

// Implements sol_memmove_ syscall for overlapping memory moves.
// Similar to memcpy but handles overlapping regions correctly.
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

// Implements sol_memset_ syscall for memory initialization.
// Fills memory region with specified byte value.
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

// Implements sol_memcmp_ syscall for memory comparison.
// Returns -1, 0, or 1 based on lexicographic comparison.
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

/// Registers all implemented Solana syscalls with the SBPF loader.
/// These syscalls provide the runtime interface for Solana programs.
pub fn register_syscalls(
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
