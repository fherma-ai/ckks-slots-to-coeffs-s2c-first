//! CHECK — the output's precision against the message it was made from. NOT
//! measured as score; timed apart and reported as metrics.
//!
//! Correctness on the platform is the digest. This is the other reading of a
//! SlotsToCoeffs: how faithfully the input's slot values arrive as the
//! output's coefficients. The stage is the inverse of CoeffsToSlots, whose
//! order Poulpy's own test fixes (`test_suite::bootstrapping`, `C2S-PREC`):
//! slot `j` of the real half holds coefficient `bitrev(j)`, of the imaginary
//! half coefficient `N/2 + bitrev(j)`. So here coefficient `bitrev(j)` of the
//! output must hold the real part of slot `j` of the message, and coefficient
//! `N/2 + bitrev(j)` its imaginary part. The measure is the scale-invariant
//! signal-to-noise ratio in bits, Poulpy's `snr_bits`. The specification's
//! file; it has the secret because `setup` does.

use poulpy_ckks::api::CKKSDecryptOps;
use poulpy_ckks::layouts::{CKKSModuleAlloc, CKKSPlaintextVecHostCodec};
use poulpy_ckks::{CKKSInfos, CKKSMeta, SetCKKSInfos, SlotsKind};
use poulpy_core::layouts::LWEInfos;
use poulpy_hal::api::ScratchOwnedBorrow;

use super::keys::{Context, Ct};

/// Plaintext budget bits above `log_delta` a ciphertext is decrypted at, as
/// in Poulpy's driver.
const LOG_BUDGET: usize = 8;

/// Signal-to-noise ratio in bits of the coefficients that must hold the real
/// parts of the message, and of those that must hold the imaginary parts.
#[derive(Clone, Copy, Debug)]
pub struct Precision {
    pub re_snr_bits: f64,
    pub im_snr_bits: f64,
}

/// Decrypts the output to its coefficients and measures them against the message.
pub fn precision(state: &Context, output: &Ct, want_re: &[f64], want_im: &[f64]) -> Precision {
    let coeffs = coefficients(state, output);
    let m = coeffs.len() / 2;
    let bits = m.trailing_zeros() as usize;
    let (mut got_re, mut got_im) = (vec![0f64; m], vec![0f64; m]);
    for j in 0..m {
        let b = bitrev(j, bits);
        got_re[j] = coeffs[b];
        got_im[j] = coeffs[m + b];
    }
    Precision {
        re_snr_bits: snr_bits(&got_re, want_re),
        im_snr_bits: snr_bits(&got_im, want_im),
    }
}

/// The coefficients a ciphertext encrypts, as floats at its scale.
pub fn coefficients(state: &Context, ct: &Ct) -> Vec<f64> {
    let log_delta = ct.log_delta();
    let log_budget = ct
        .log_budget()
        .min(LOG_BUDGET)
        .min(127usize.saturating_sub(log_delta));
    let mut pt = state
        .module
        .ckks_pt_vec_alloc(ct.base2k(), (log_delta + log_budget).into());
    pt.set_meta(CKKSMeta {
        log_sparsity: 0,
        log_delta,
        slots: SlotsKind::Complex,
    });
    let mut arena = state.scratch.borrow_mut();
    state
        .module
        .ckks_decrypt(&mut pt, ct, &state.sk, &mut arena.borrow())
        .expect("decrypt for the precision check");
    let mut coeffs = vec![0f64; ct.n().as_usize()];
    pt.decode_host_floats(&mut coeffs)
        .expect("read the coefficients for the precision check");
    coeffs
}

/// Bit-reversal of `j` over `bits` bits: Poulpy's slot-to-coefficient order.
pub fn bitrev(j: usize, bits: usize) -> usize {
    ((j as u32).reverse_bits() >> (u32::BITS - bits as u32)) as usize
}

/// Scale-invariant signal-to-noise ratio in bits, Poulpy's `snr_bits`: the
/// best global scale `s` between `got` and `want`, then
/// `-0.5·log2(||got − s·want||² / ||s·want||²)`.
pub fn snr_bits(got: &[f64], want: &[f64]) -> f64 {
    let dot_gw: f64 = got.iter().zip(want).map(|(g, w)| g * w).sum();
    let dot_ww: f64 = want.iter().map(|w| w * w).sum();
    let s = if dot_ww > 0.0 { dot_gw / dot_ww } else { 0.0 };
    let err2: f64 = got.iter().zip(want).map(|(g, w)| (g - s * w).powi(2)).sum();
    let sig2: f64 = want.iter().map(|w| (s * w).powi(2)).sum();
    if err2 <= 0.0 || sig2 <= 0.0 {
        return f64::INFINITY;
    }
    -0.5 * (err2 / sig2).log2()
}
