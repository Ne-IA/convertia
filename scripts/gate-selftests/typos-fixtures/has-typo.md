# G51 typos self-test fixture (planted positive)

This file is scanned ONLY by the g24-typos self-test, never by the live G51 gate (the gate's scope
is the public-facing prose, not scripts/gate-selftests/). It carries deliberate misspellings the
pinned typos binary MUST flag:

- teh quick brown fox
- please recieve the seperate occurence

It also carries a `mis-stripped` token: the `.typos.toml` allowlist (`mis = "mis"`) must keep typos
from flagging the valid `mis-` prefix, while it STILL flags `teh`/`recieve` above.
