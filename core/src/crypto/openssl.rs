//! Linux 通过已安装的 OpenSSL 3 libcrypto 完成 AES-256-GCM、HMAC 与 PBKDF2。

use std::io::{self, ErrorKind};
use std::ptr;

use super::crypto_error;

#[repr(C)]
struct EvpCipherCtx {
    _private: [u8; 0],
}
#[repr(C)]
struct EvpCipher {
    _private: [u8; 0],
}
#[repr(C)]
struct EvpMd {
    _private: [u8; 0],
}

const EVP_CTRL_GCM_SET_IVLEN: i32 = 0x9;
const EVP_CTRL_GCM_GET_TAG: i32 = 0x10;
const EVP_CTRL_GCM_SET_TAG: i32 = 0x11;

#[link(name = "libcrypto.so.3", kind = "dylib", modifiers = "+verbatim")]
unsafe extern "C" {
    fn EVP_CIPHER_CTX_new() -> *mut EvpCipherCtx;
    fn EVP_CIPHER_CTX_free(ctx: *mut EvpCipherCtx);
    fn EVP_EncryptInit_ex(
        ctx: *mut EvpCipherCtx,
        kind: *const EvpCipher,
        engine: *mut (),
        key: *const u8,
        iv: *const u8,
    ) -> i32;
    fn EVP_DecryptInit_ex(
        ctx: *mut EvpCipherCtx,
        kind: *const EvpCipher,
        engine: *mut (),
        key: *const u8,
        iv: *const u8,
    ) -> i32;
    fn EVP_EncryptUpdate(
        ctx: *mut EvpCipherCtx,
        out: *mut u8,
        out_len: *mut i32,
        input: *const u8,
        input_len: i32,
    ) -> i32;
    fn EVP_DecryptUpdate(
        ctx: *mut EvpCipherCtx,
        out: *mut u8,
        out_len: *mut i32,
        input: *const u8,
        input_len: i32,
    ) -> i32;
    fn EVP_EncryptFinal_ex(ctx: *mut EvpCipherCtx, out: *mut u8, out_len: *mut i32) -> i32;
    fn EVP_DecryptFinal_ex(ctx: *mut EvpCipherCtx, out: *mut u8, out_len: *mut i32) -> i32;
    fn EVP_CIPHER_CTX_ctrl(ctx: *mut EvpCipherCtx, op: i32, arg: i32, ptr: *mut u8) -> i32;
    fn EVP_aes_256_gcm() -> *const EvpCipher;
    fn EVP_sha256() -> *const EvpMd;
    fn PKCS5_PBKDF2_HMAC(
        password: *const u8,
        password_len: i32,
        salt: *const u8,
        salt_len: i32,
        iterations: i32,
        digest: *const EvpMd,
        key_len: i32,
        out: *mut u8,
    ) -> i32;
    fn HMAC(
        digest: *const EvpMd,
        key: *const u8,
        key_len: i32,
        data: *const u8,
        data_len: usize,
        out: *mut u8,
        out_len: *mut u32,
    ) -> *mut u8;
    fn RAND_bytes(buf: *mut u8, len: i32) -> i32;
}

struct Context(*mut EvpCipherCtx);

impl Context {
    fn new() -> io::Result<Self> {
        let ctx = unsafe { EVP_CIPHER_CTX_new() };
        if ctx.is_null() {
            Err(crypto_error("OpenSSL could not create a cipher context"))
        } else {
            Ok(Self(ctx))
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { EVP_CIPHER_CTX_free(self.0) };
    }
}

fn ok(status: i32) -> io::Result<()> {
    if status == 1 {
        Ok(())
    } else {
        Err(crypto_error("OpenSSL rejected a cryptographic operation"))
    }
}

fn bytes_or_empty(bytes: &[u8]) -> *const u8 {
    if bytes.is_empty() {
        ptr::null()
    } else {
        bytes.as_ptr()
    }
}

pub(crate) fn random_bytes(buf: &mut [u8]) -> io::Result<()> {
    if buf.is_empty() {
        return Ok(());
    }
    ok(unsafe {
        RAND_bytes(
            buf.as_mut_ptr(),
            i32::try_from(buf.len()).map_err(|_| length())?,
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
    ok(unsafe {
        PKCS5_PBKDF2_HMAC(
            bytes_or_empty(password),
            i32::try_from(password.len()).map_err(|_| length())?,
            bytes_or_empty(salt),
            i32::try_from(salt.len()).map_err(|_| length())?,
            i32::try_from(iterations).map_err(|_| length())?,
            EVP_sha256(),
            i32::try_from(out.len()).map_err(|_| length())?,
            out.as_mut_ptr(),
        )
    })
}

pub(crate) fn hmac_sha256(key: &[u8], data: &[u8]) -> io::Result<[u8; 32]> {
    let mut out = [0u8; 32];
    let mut len = 0u32;
    let result = unsafe {
        HMAC(
            EVP_sha256(),
            bytes_or_empty(key),
            i32::try_from(key.len()).map_err(|_| length())?,
            bytes_or_empty(data),
            data.len(),
            out.as_mut_ptr(),
            &mut len,
        )
    };
    if result.is_null() || len != 32 {
        return Err(crypto_error("OpenSSL could not compute HMAC-SHA256"));
    }
    Ok(out)
}

fn start(ctx: &Context, encrypt: bool, key: &[u8; 32], nonce: &[u8; 12]) -> io::Result<()> {
    let init = if encrypt {
        EVP_EncryptInit_ex
    } else {
        EVP_DecryptInit_ex
    };
    ok(unsafe {
        init(
            ctx.0,
            EVP_aes_256_gcm(),
            ptr::null_mut(),
            ptr::null(),
            ptr::null(),
        )
    })?;
    ok(unsafe { EVP_CIPHER_CTX_ctrl(ctx.0, EVP_CTRL_GCM_SET_IVLEN, 12, ptr::null_mut()) })?;
    ok(unsafe {
        init(
            ctx.0,
            ptr::null(),
            ptr::null_mut(),
            key.as_ptr(),
            nonce.as_ptr(),
        )
    })
}

fn absorb(ctx: &Context, encrypt: bool, aad: &[u8]) -> io::Result<()> {
    if aad.is_empty() {
        return Ok(());
    }
    let mut ignored = 0i32;
    let update = if encrypt {
        EVP_EncryptUpdate
    } else {
        EVP_DecryptUpdate
    };
    ok(unsafe {
        update(
            ctx.0,
            ptr::null_mut(),
            &mut ignored,
            aad.as_ptr(),
            i32::try_from(aad.len()).map_err(|_| length())?,
        )
    })
}

pub(crate) fn aead_seal(
    key: &[u8; 32],
    nonce: &[u8; 12],
    aad: &[u8],
    plain: &[u8],
) -> io::Result<Vec<u8>> {
    let ctx = Context::new()?;
    start(&ctx, true, key, nonce)?;
    absorb(&ctx, true, aad)?;
    let mut out = vec![0u8; plain.len()];
    let mut len = 0i32;
    if !plain.is_empty() {
        ok(unsafe {
            EVP_EncryptUpdate(
                ctx.0,
                out.as_mut_ptr(),
                &mut len,
                plain.as_ptr(),
                i32::try_from(plain.len()).map_err(|_| length())?,
            )
        })?;
    }
    let mut final_len = 0i32;
    let mut extra = [0u8; 16];
    ok(unsafe { EVP_EncryptFinal_ex(ctx.0, extra.as_mut_ptr(), &mut final_len) })?;
    if final_len != 0 || len as usize != plain.len() {
        return Err(crypto_error("AES-GCM returned an unexpected length"));
    }
    let mut tag = [0u8; 16];
    ok(unsafe { EVP_CIPHER_CTX_ctrl(ctx.0, EVP_CTRL_GCM_GET_TAG, 16, tag.as_mut_ptr()) })?;
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
    let (ciphertext, tag) = stored.split_at(stored.len() - 16);
    let ctx = Context::new()?;
    start(&ctx, false, key, nonce)?;
    absorb(&ctx, false, aad)?;
    let mut out = vec![0u8; ciphertext.len()];
    let mut len = 0i32;
    if !ciphertext.is_empty() {
        ok(unsafe {
            EVP_DecryptUpdate(
                ctx.0,
                out.as_mut_ptr(),
                &mut len,
                ciphertext.as_ptr(),
                i32::try_from(ciphertext.len()).map_err(|_| length())?,
            )
        })?;
    }
    ok(unsafe { EVP_CIPHER_CTX_ctrl(ctx.0, EVP_CTRL_GCM_SET_TAG, 16, tag.as_ptr() as *mut u8) })?;
    let mut final_len = 0i32;
    let mut extra = [0u8; 16];
    if unsafe { EVP_DecryptFinal_ex(ctx.0, extra.as_mut_ptr(), &mut final_len) } != 1 {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "sealed record failed authentication",
        ));
    }
    if final_len != 0 || len as usize != ciphertext.len() {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "sealed record failed authentication",
        ));
    }
    Ok(out)
}
