/// Minimal runtime types for Solana program execution in zkVM.

/// Represents a Solana account with all necessary metadata.
/// Mirrors the on-chain account structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
    pub lamports: u64,
    pub data: Vec<u8>,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: u64,
}

/// 32-byte public key used throughout Solana.
/// Supports base58 string conversion for human-readable addresses.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Pubkey([u8; 32]);

impl Pubkey {
    /// Returns the underlying byte array.
    pub(crate) fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<String> for Pubkey {
    type Error = String;

    /// Converts a base58-encoded string to a Pubkey.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match bs58::decode(value.clone()).into_vec() {
            Ok(bytes) => {
                if bytes.len() == 32 {
                    let mut array = [0u8; 32];
                    array.copy_from_slice(&bytes);
                    Ok(Self(array))
                } else {
                    Err(format!(
                        "Invalid pubkey '{}' length: {}",
                        value,
                        bytes.len()
                    ))
                }
            }
            Err(_) => Err("Invalid base58 encoding".into()),
        }
    }
}
