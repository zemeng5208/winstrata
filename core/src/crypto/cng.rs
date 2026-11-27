//! Windows 通过 CNG 的 BCrypt 完成 AES-256-GCM、HMAC 与 PBKDF2。

use std::io::{self, ErrorKind};
use std::ptr::{null, null_mut};

use windows_sys::Win32::Security::Cryptography::{
    BCRYPT_AES_ALGORITHM, BCRYPT_ALG_HANDLE, BCRYPT_ALG_HANDLE_HMAC_FLAG,
    BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO, BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION,
    BCRYPT_CHAIN_MODE_GCM, BCRYPT_CHAINING_MODE, BCRYPT_HASH_HANDLE, BCRYPT_KEY_HANDLE,
    BCRYPT_OBJECT_LENGTH, BCRYPT_SHA256_ALGORITHM, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDecrypt, BCryptDeriveKeyPBKDF2,
    BCryptDestroyHash, BCryptDestroyKey, BCryptEncrypt, BCryptFinishHash, BCryptGenRandom,
    BCryptGenerateSymmetricKey, BCryptGetProperty, BCryptHashData, BCryptOpenAlgorithmProvider,
    BCryptSetProperty,
};

use super::crypto_error;

struct Algorithm(BCRYPT_ALG_HANDLE);

impl Drop for Algorithm {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { BCryptCloseAlgorithmProvider(self.0, 0) };
        }
    }
}

struct Key {
    handle: BCRYPT_KEY_HANDLE,
    object: Vec<u8>,
}

impl Drop for Key {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { BCryptDestroyKey(self.handle) };
        }
        for byte in &mut self.object {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
    }
}

struct Hash(BCRYPT_HASH_HANDLE);

impl Drop for Hash {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { BCryptDestroyHash(self.0) };
        }
    }
}

fn status_ok(status: i32) -> io::Result<()> {
    if status >= 0 {
        Ok(())
    } else {
        Err(crypto_error("CNG rejected a cryptographic operation"))
    }
}

fn open_algorithm(id: windows_sys::core::PCWSTR, flags: u32) -> io::Result<Algorithm> {
    let mut handle = null_mut();
    status_ok(unsafe { BCryptOpenAlgorithmProvider(&mut handle, id, null(), flags) })?;
    Ok(Algorithm(handle))
}

fn wide_bytes(value: windows_sys::core::PCWSTR) -> u32 {
    let mut units = 0usize;
    // SAFETY: windows-sys 的宽字符串字面量以 NUL 结尾，并且在程序生命周期内有效。
    unsafe {
        while *value.add(units) != 0 {
            units += 1;
        }
    }
    u32::try_from((units + 1) * 2).unwrap_or(u32::MAX)
}

fn set_gcm(algorithm: &Algorithm) -> io::Result<()> {
    status_ok(unsafe {
        BCryptSetProperty(
            algorithm.0,
            BCRYPT_CHAINING_MODE,
            BCRYPT_CHAIN_MODE_GCM.cast(),
            wide_bytes(BCRYPT_CHAIN_MODE_GCM),
            0,
        )
    })
}

fn property_u32(
    handle: *mut core::ffi::c_void,
    name: windows_sys::core::PCWSTR,
) -> io::Result<u32> {
    let mut value = 0u32;
    let mut copied = 0u32;
    status_ok(unsafe {
        BCryptGetProperty(
            handle,
            name,
            (&mut value as *mut u32).cast(),
            4,
            &mut copied,
            0,
        )
    })?;
    if copied != 4 {
        return Err(crypto_error("CNG returned an unexpected property"));
    }
    Ok(value)
}

fn symmetric_key(algorithm: &Algorithm, secret: &[u8; 32]) -> io::Result<Key> {
    let length = property_u32(algorithm.0, BCRYPT_OBJECT_LENGTH)? as usize;
    let mut object = vec![0u8; length];
    let mut handle = null_mut();
    status_ok(unsafe {
        BCryptGenerateSymmetricKey(
            algorithm.0,
            &mut handle,
            object.as_mut_ptr(),
            u32::try_from(object.len()).unwrap_or(u32::MAX),
            secret.as_ptr(),
            32,
            0,
        )
    })?;
    Ok(Key { handle, object })
}

pub(crate) fn random_bytes(buf: &mut [u8]) -> io::Result<()> {
    if buf.is_empty() {
        return Ok(());
    }
    status_ok(unsafe {
        BCryptGenRandom(
            null_mut(),
            buf.as_mut_ptr(),
            u32::try_from(buf.len()).map_err(|_| length())?,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    })
}

fn length() -> io::Error {
    io::Error::new(ErrorKind::InvalidInput, "cryptographic input is too large")
}

pub(crate) fn pbkdf2(
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    out: &mut [u8],
) -> io::Result<()> {
    if iterations == 0 || out.is_empty() {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "PBKDF2 parameters are incomplete",
        ));
    }
    let algorithm = open_algorithm(BCRYPT_SHA256_ALGORITHM, BCRYPT_ALG_HANDLE_HMAC_FLAG)?;
    status_ok(unsafe {
        BCryptDeriveKeyPBKDF2(
            algorithm.0,
            if password.is_empty() {
                null()
            } else {
                password.as_ptr()
            },
            u32::try_from(password.len()).map_err(|_| length())?,
            if salt.is_empty() {
                null()
            } else {
                salt.as_ptr()
            },
            u32::try_from(salt.len()).map_err(|_| length())?,
            u64::from(iterations),
            out.as_mut_ptr(),
            u32::try_from(out.len()).map_err(|_| length())?,
            0,
        )
    })
}

pub(crate) fn hmac_sha256(key: &[u8], data: &[u8]) -> io::Result<[u8; 32]> {
    let algorithm = open_algorithm(BCRYPT_SHA256_ALGORITHM, BCRYPT_ALG_HANDLE_HMAC_FLAG)?;
    let object_len = property_u32(algorithm.0, BCRYPT_OBJECT_LENGTH)? as usize;
    let mut object = vec![0u8; object_len];
    let mut handle = null_mut();
    status_ok(unsafe {
        BCryptCreateHash(
            algorithm.0,
            &mut handle,
            object.as_mut_ptr(),
            u32::try_from(object.len()).unwrap_or(u32::MAX),
            if key.is_empty() { null() } else { key.as_ptr() },
            u32::try_from(key.len()).map_err(|_| length())?,
            0,
        )
    })?;
    let hash = Hash(handle);
    if !data.is_empty() {
        status_ok(unsafe {
            BCryptHashData(
                hash.0,
                data.as_ptr(),
                u32::try_from(data.len()).map_err(|_| length())?,
                0,
            )
        })?;
    }
    let mut out = [0u8; 32];
    status_ok(unsafe { BCryptFinishHash(hash.0, out.as_mut_ptr(), 32, 0) })?;
    Ok(out)
}

fn auth_info<'a>(
    nonce: &'a mut [u8; 12],
    aad: &'a mut [u8],
    tag: &'a mut [u8; 16],
) -> BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
    BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
        cbSize: std::mem::size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32,
        dwInfoVersion: BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION,
        pbNonce: nonce.as_mut_ptr(),
        cbNonce: 12,
        pbAuthData: if aad.is_empty() {
            null_mut()
        } else {
            aad.as_mut_ptr()
        },
        cbAuthData: aad.len() as u32,
        pbTag: tag.as_mut_ptr(),
        cbTag: 16,
        pbMacContext: null_mut(),
        cbMacContext: 0,
        cbAAD: 0,
        cbData: 0,
        dwFlags: 0,
    }
}

pub(crate) fn aead_seal(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    plain: &[u8],
) -> io::Result<Vec<u8>> {
    let algorithm = open_algorithm(BCRYPT_AES_ALGORITHM, 0)?;
    set_gcm(&algorithm)?;
    let key = symmetric_key(&algorithm, key)?;
    let mut nonce = *nonce;
    let mut aad = aad.to_vec();
    let mut tag = [0u8; 16];
    let mut info = auth_info(&mut nonce, &mut aad, &mut tag);
    let mut out = vec![0u8; plain.len()];
    let mut written = 0u32;
    // 空明文仍要算出认证标签。NULL 输出会被 CNG 当成“只查询长度”。
    let mut empty_out = [0u8; 1];
    let (output, output_len) = if out.is_empty() {
        (empty_out.as_mut_ptr(), 1)
    } else {
        (out.as_mut_ptr(), u32::try_from(out.len()).unwrap_or(0))
    };
    status_ok(unsafe {
        BCryptEncrypt(
            key.handle,
            if plain.is_empty() {
                null()
            } else {
                plain.as_ptr()
            },
            u32::try_from(plain.len()).map_err(|_| length())?,
            (&mut info as *mut BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO).cast(),
            null_mut(),
            0,
            output,
            output_len,
            &mut written,
            0,
        )
    })?;
    if written as usize != plain.len() {
        return Err(crypto_error("AES-GCM returned an unexpected length"));
    }
    out.extend_from_slice(&tag);
    Ok(out)
}

pub(crate) fn aead_open(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    stored: &[u8],
) -> io::Result<Vec<u8>> {
    if stored.len() < 16 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "sealed record failed authentication",
        ));
    }
    let (ciphertext, tag_bytes) = stored.split_at(stored.len() - 16);
    let algorithm = open_algorithm(BCRYPT_AES_ALGORITHM, 0)?;
    set_gcm(&algorithm)?;
    let key = symmetric_key(&algorithm, key)?;
    let mut nonce = *nonce;
    let mut aad = aad.to_vec();
    let mut tag = [0u8; 16];
    tag.copy_from_slice(tag_bytes);
    let mut info = auth_info(&mut nonce, &mut aad, &mut tag);
    let mut out = vec![0u8; ciphertext.len()];
    let mut written = 0u32;
    let mut empty_out = [0u8; 1];
    let (output, output_len) = if out.is_empty() {
        (empty_out.as_mut_ptr(), 1)
    } else {
        (out.as_mut_ptr(), u32::try_from(out.len()).unwrap_or(0))
    };
    let status = unsafe {
        BCryptDecrypt(
            key.handle,
            if ciphertext.is_empty() {
                null()
            } else {
                ciphertext.as_ptr()
            },
            u32::try_from(ciphertext.len()).map_err(|_| length())?,
            (&mut info as *mut BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO).cast(),
            null_mut(),
            0,
            output,
            output_len,
            &mut written,
            0,
        )
    };
    if status < 0 || written as usize != ciphertext.len() {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "sealed record failed authentication",
        ));
    }
    Ok(out)
}
