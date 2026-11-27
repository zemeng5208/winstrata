//! 记录加密使用操作系统提供的 AES-256-GCM 与 PBKDF2-HMAC-SHA256。
//!
//! Linux 调用本机 OpenSSL 3 的 libcrypto，Windows 调用 CNG。两边处理同一组
//! 公开测试向量。这里不实现分组密码本身。

#[cfg(target_os = "linux")]
#[path = "openssl.rs"]
mod platform;
#[cfg(windows)]
#[path = "cng.rs"]
mod platform;

use std::io::{self, ErrorKind};
use std::sync::atomic::{Ordering, compiler_fence};

pub(crate) use platform::{aead_open, aead_seal, hmac_sha256, pbkdf2, random_bytes};

pub(crate) fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    compiler_fence(Ordering::SeqCst);
}

pub(crate) fn crypto_error(detail: &str) -> io::Error {
    io::Error::new(ErrorKind::Other, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_gcm_matches_the_published_vectors() {
        let mut key = [0u8; 32];
        let nonce = [0u8; 12];
        let plain = [0u8; 16];
        let sealed = aead_seal(&key, &nonce, &[], &plain).unwrap();
        assert_eq!(
            sealed,
            hex("cea7403d4d606b6e074ec5d3baf39d18d0d1c8a799996bf0265b98b5d48ab919")
        );
        assert_eq!(aead_open(&key, &nonce, &[], &sealed).unwrap(), plain);

        for (index, byte) in key.iter_mut().enumerate() {
            *byte = index as u8;
        }
        let nonce = [100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111];
        let sealed = aead_seal(&key, &nonce, &[1, 2, 3, 4], b"hello").unwrap();
        assert_eq!(sealed, hex("207eb20a165eb06a7d29b441039301f2283303e85a"));
        assert_eq!(
            aead_open(&key, &nonce, &[1, 2, 3, 4], &sealed).unwrap(),
            b"hello"
        );
        let mut tampered = sealed.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert_eq!(
            aead_open(&key, &nonce, &[1, 2, 3, 4], &tampered)
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidData
        );
        assert!(aead_open(&key, &nonce, &[1, 2, 3, 4, 5], &sealed).is_err());
    }

    #[test]
    fn pbkdf2_and_hmac_match_the_published_vectors() {
        let mut derived = [0u8; 32];
        pbkdf2(b"password", b"salt", 1, &mut derived).unwrap();
        assert_eq!(
            derived.as_slice(),
            hex("120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b").as_slice()
        );
        let mac = hmac_sha256(&derived, b"winstrata-password-check-v1").unwrap();
        assert_eq!(
            mac.as_slice(),
            hex("58e605153ef05a31a24893c7b06a6685e7c1310e872775dd78a94d890fba6c36").as_slice()
        );
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }
}
