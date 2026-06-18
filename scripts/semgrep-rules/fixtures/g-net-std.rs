// PLANTED-POSITIVE armed canary for G29 — this file DELIBERATELY violates the named rule and
// MUST be flagged by it (the SAST self-test prelude asserts it). DO NOT "fix" it. This dir is L(-1).
// rule (g): convertia-net-ban-std-tokio (forbidden std::net import)
use std::net::TcpStream;

fn connect() {
    let _ = TcpStream::connect("127.0.0.1:80");
}
