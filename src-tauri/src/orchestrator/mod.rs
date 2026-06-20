//! `crate::orchestrator` — the §1.9 batch / job-lifecycle conductor: it builds the queue, drives
//! `JobState`, holds the run registry + cancellation tokens (§0.4.4), and fans progress out to the
//! Channel. It sequences the guarantees / engines / detection layers; it owns none of their
//! behaviour. Filled by P3.46.
