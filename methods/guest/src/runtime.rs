// A very minimal runtime
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

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Pubkey([u8; 32]);

impl Pubkey {
    pub(crate) fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl TryFrom<String> for Pubkey {
    type Error = String;

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
