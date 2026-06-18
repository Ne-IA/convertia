// PLANTED-POSITIVE armed canary for G29 — this file DELIBERATELY violates the named rule and
// MUST be flagged by it (the SAST self-test prelude asserts it). DO NOT "fix" it. This dir is L(-1).
// rule (j): convertia-net-ban-raw-socket-ffi (forbidden raw libc socket)
fn raw_socket() {
    unsafe {
        let _fd = libc::socket(2, 1, 0);
    }
}
