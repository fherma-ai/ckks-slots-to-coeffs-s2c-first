//! RUN — the function under measurement. The loop times this call and
//! nothing else.
//!
//! Reference: Poulpy's SlotsToCoeffs as the S2C-first bootstrap performs it
//! (`ckks_bootstrap_s2c_mod_up` in Poulpy, before its ModUp): a copy of the
//! input doubled — the split decode matrix reconstructs `2·ct`, and the
//! orchestrator keeps that normalisation — then the homomorphic decode DFT
//! `ckks_dft_evaluate_assign` on the preset's compiled SlotsToCoeffs matrix
//! (four factors at scale 2²⁸) with the rotation keys.
//!
//! A submission with its own algorithm replaces the body of `run`. It gets
//! its state from `init` and the input, and must leave the same bytes in the
//! output as this reference does.

use poulpy_ckks::api::{CKKSCopyOps, CKKSDFTOps, CKKSPow2Ops};
use poulpy_ckks::layouts::BootstrappingKeys;
use poulpy_ckks::SetCKKSInfos;
use poulpy_hal::api::ScratchOwnedBorrow;

use crate::envelope::{Input, Output};
use crate::init::State;

pub fn run<'a>(state: &'a mut State<'_>, input: &Input) -> &'a Output {
    let context = state.context;
    let State { output, scratch, .. } = state;
    let mut scratch = scratch.borrow();

    // The transform consumes width: it starts at the input width and leaves
    // the output narrower. The buffer is reused, so it is set back to the
    // input width before every run.
    output.set_k(context.preset.input_k().into());
    context.module.ckks_copy(output, &input.ct, &mut scratch).expect("copy the input");
    context
        .module
        .ckks_mul_pow2_assign(output, 1, &mut scratch)
        .expect("double the input");
    context
        .module
        .ckks_dft_evaluate_assign(
            output,
            context.context.slots_to_coeffs(),
            context.keys.rotation_keys(),
            &mut scratch,
        )
        .expect("SlotsToCoeffs");
    output
}
