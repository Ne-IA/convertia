//! `crate::isolation` — the §2.12 decoder-isolation wrapper every engine spawn routes through. This
//! is the SOLE sanctioned `std::process::Command::new` site in the codebase (the G9 repo-invariant
//! scopes its `Command::new` grep here), and the home of the §2.12.3 per-OS privilege-drop tiers. The
//! interface shell lands in P3.2; the real confined-spawn wrapper in P4.13.
