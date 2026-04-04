use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::{Debug, Display, Formatter};
use base64::Engine;
use serde::{Deserialize, Serialize};
use crate::result;
use crate::result::{Error, ErrorType};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct OptInfo {
    pub opt: Vec<String>,
    pub debug: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct GitInfo {
    pub url: String,
    pub branch: String,
    pub dirty: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct BuilderInfo {
    pub version: String,
    pub info: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DeveloperInfo {
    pub developer: String,
    pub link: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct BuildInfo {
    pub name: String,
    pub features: Vec<String>,
    pub profile: String,
    pub cycle: String,
    pub version: String,
}

#[repr(C)]
#[derive(Eq, PartialEq)]
pub struct HashFnData {
    pub ptr: *const u8,
    pub len: u64,
}

#[repr(C)]
#[derive(Eq, PartialEq)]

pub struct HashFnArgs {
    pub args: *const *const u8,
    pub str_len: *const u32,
    pub item_len: u16,
}

#[repr(C)]
#[derive(Eq, PartialEq)]

pub struct LenResult {
    pub success: bool,
    pub len: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum HashVariant {
    Sha1(Vec<u8>),
    Sha1Dc(Vec<u8>),

    Sha2_256(Vec<u8>),
    Sha2_512(Vec<u8>),
    Sha2_512_256(Vec<u8>),

    Sha3_256(Vec<u8>),
    Sha3_512(Vec<u8>),

    Blake2B(Vec<u8>),
    Blake2S(Vec<u8>),

    Blake3(Vec<u8>),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Eq, PartialEq)]
pub struct HashInfo {
    pub dir_hash: Vec<HashVariant>,
    pub git_hash: Vec<HashVariant>,
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct VersionInfo {
    pub data_version: u32,
    pub git: Option<GitInfo>,
    pub builder: Option<BuilderInfo>,
    pub developer: DeveloperInfo,
    pub build: BuildInfo,
    pub hash: HashInfo,
    pub additional: BTreeMap<String, String>,
}

impl Display for HashVariant {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.algo(), self.hash())
    }
}

impl HashVariant {
    #[inline]
    pub fn hash(&self) -> String {
        let bytes = match self {
            HashVariant::Sha1(b) | HashVariant::Sha1Dc(b) |
            HashVariant::Sha2_256(b) | HashVariant::Sha2_512(b) |
            HashVariant::Sha2_512_256(b) | HashVariant::Sha3_256(b) |
            HashVariant::Sha3_512(b) | HashVariant::Blake2B(b) |
            HashVariant::Blake2S(b) | HashVariant::Blake3(b) => b,
        };

        base64::prelude::BASE64_URL_SAFE.encode(bytes)
    }

    #[inline]
    pub fn algo(&self) -> String {
        match self {
            HashVariant::Sha1(_) => "SHA-1",
            HashVariant::Sha1Dc(_) => "SHA-1DC",

            HashVariant::Sha2_256(_) => "SHA2-256",
            HashVariant::Sha2_512(_) => "SHA2-512",
            HashVariant::Sha2_512_256(_) => "SHA2-512/256",

            HashVariant::Sha3_256(_) => "SHA3-256",
            HashVariant::Sha3_512(_) => "SHA3-512",

            HashVariant::Blake2B(_) => "Blake2_B",
            HashVariant::Blake2S(_) => "Blake2_S",

            HashVariant::Blake3(_) => "Blake3",
            #[allow(unreachable_patterns)]
            _ => "Unknown"
        }.to_string()
    }

    pub fn from_parts(algo: &str, hash: Vec<u8>) -> result::Result<HashVariant> {
        match algo {
            "SHA-1" => Ok(HashVariant::Sha1(hash)),
            "SHA-1DC" => Ok(HashVariant::Sha1Dc(hash)),
            "SHA2-256" => Ok(HashVariant::Sha2_256(hash)),
            "SHA2-512" => Ok(HashVariant::Sha2_512(hash)),
            "SHA2-512/256" => Ok(HashVariant::Sha2_512_256(hash)),
            "SHA3-256" => Ok(HashVariant::Sha3_256(hash)),
            "SHA3-512" => Ok(HashVariant::Sha3_512(hash)),
            "Blake2_B" => Ok(HashVariant::Blake2B(hash)),
            "Blake2_S" => Ok(HashVariant::Blake2S(hash)),
            "Blake3" => Ok(HashVariant::Blake3(hash)),
            _ => Error::new_string(
                ErrorType::NotSupported,
                Some(format!("not supported hash type ({})", algo)),
            ).raise(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum EcdsaType {
    P256,
    P384,
    P521,
    BrainPool(u16),
}

#[derive(Debug, Clone)]
pub enum SignVariant {
    RsaPss2048(HashVariant, String),
    RsaPss4096(HashVariant, String),
    ECDSA(EcdsaType, HashVariant, String),
    ED25519(String),
    ED448(String),
    DILITHIUM(u8, String)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use crate::version::OS_VERSION;

    #[test]
    fn test_version_info_roundtrip() {
        let data = yaml_peg::serde::to_string(&*OS_VERSION).unwrap();

        let decoded: Vec<VersionInfo> = yaml_peg::serde::from_str(&data).expect("Failed to deserialize");

        if decoded.len() != 1 {
            panic!("len is bad")
        }

        let decoded = decoded.get(0).unwrap();

        assert_eq!(decoded, &*OS_VERSION);
    }
}