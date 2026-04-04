use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use const_format::formatcp;
use const_str::split;

pub const VERSION_DATA_VERSION: u32 = 1;

use super::types::{
    BuildInfo,
    BuilderInfo,
    DeveloperInfo,
    GitInfo,
    HashInfo,
    HashVariant,
    VersionInfo
};

use crate::{MICRO_VER, OS_NAME, VERSION_RAW};

const fn hex_to_byte(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("Invalid hex char"),
    }
}

macro_rules! decode_hex {
    ($s:expr) => {{
        const S: &[u8] = $s.as_bytes();
        const LEN: usize = S.len() / 2;
        const OUT: [u8; LEN] = {
            let mut res = [0u8; LEN];
            let mut i = 0;
            while i < LEN {
                // 2文字(ASCII)を1バイト(数値)に結合
                res[i] = (hex_to_byte(S[i * 2]) << 4) | hex_to_byte(S[i * 2 + 1]);
                i += 1;
            }
            res
        };
        &OUT
    }};
}

const DIR_HASH: &[u8] = decode_hex!(env!("DIR_HASH"));
const GIT_HASH: &[u8] = decode_hex!(env!("GIT_HASH"));

const FEATURES: &[&str] = &split!(env!("BUILD_FEATURES"), ",");


impl VersionInfo {
    pub fn new_os() -> VersionInfo {
            let build = BuildInfo {
                name: OS_NAME.to_string(),
                features: Vec::new(),
                profile: env!("OS_PROFILE").to_string(),
                cycle: env!("OS_CYCLE").to_string(),
                version: formatcp!("{VERSION_RAW}_{MICRO_VER}").to_string(),
            };

            let git = GitInfo {
                url: env!("GIT_URL").to_string(),
                branch: env!("GIT_BRANCH").to_string(),
                dirty: env!("GIT_DIRTY").to_string()
            };

            let builder = BuilderInfo {
                version: env!("RUST_VER").to_string(),
                info: env!("RUST_VERSION_INFO").to_string(),
                name: "rust".to_string(),
            };

            let developer = DeveloperInfo {
                developer: env!("GIT_USER").to_string(),
                link: concat!("https://github.com/", env!("GIT_USER")).to_string(),
            };

            let hash = HashInfo {

                dir_hash: vec![
                    HashVariant::Sha3_512(DIR_HASH.to_vec())
                ],
                git_hash: vec![
                    HashVariant::Sha1Dc(GIT_HASH.to_vec())
                ],
            };

        let mut info = VersionInfo {
            data_version: VERSION_DATA_VERSION,
            git: Some(git),
            builder: Some(builder),
            developer,
            build,
            hash,
            additional: BTreeMap::new(),
        };

        let entries = [
            ("OsBuildHost".to_string(), env!("BUILD_HOST").to_string()),
            ("OsBuildTarget".to_string(), env!("BUILD_TARGET").to_string()),
        ];

        info.additional = entries.into_iter().collect::<BTreeMap<String, String>>();
        info.build.features = FEATURES.iter().map(|&s| s.to_string()).collect();

        info
    }
}