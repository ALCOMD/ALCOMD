//! Read the Darwin kernel creation timestamp for the observed IPC peer.
#![allow(unsafe_code)]

pub(super) fn darwin_creation(pid: u32) -> Option<u64> {
    use std::mem::{MaybeUninit, size_of};
    let mut info = MaybeUninit::<libc::proc_bsdinfo>::uninit();
    let size = i32::try_from(size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: PROC_PIDTBSDINFO writes at most the supplied struct size; only
    // an exact length permits reading any field from the output.
    let returned = unsafe {
        libc::proc_pidinfo(
            i32::try_from(pid).ok()?,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    if returned != size {
        return None;
    }
    // SAFETY: the kernel populated the full proc_bsdinfo including these fields.
    let info = unsafe { info.assume_init() };
    if info.pbi_pid != pid {
        return None;
    }
    info.pbi_start_tvsec
        .checked_mul(1_000_000)?
        .checked_add(info.pbi_start_tvusec)
}
