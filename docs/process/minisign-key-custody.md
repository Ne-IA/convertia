# ConvertIA — minisign Key Genesis & Custody (how the signing key is born and held)

> **The birth and safekeeping of the one key the user-facing trust substitute
> collapses to.** ConvertIA ships unsigned/unnotarized binaries (no code-signing
> certificate — out of scope, SSOT), so the *only* cryptographic trust anchor a
> downloader has is the **project minisign signature over `SHA256SUMS`** (spec
> §6.2.3): verify the signature with the committed public key `docs/minisign.pub` and
> every listed asset's hash is authenticated by that one key. This doc governs how
> that key is **generated** (genesis), **kept** (custody / backup), and **recovered
> or retired** on loss — the upstream half of the key's life.
>
> **Boundary — three docs, one key, no overlap:**
> - **This doc** — *genesis + custody + the loss-recovery decision path* (how the key
>   is born and held; what to do when a working copy is lost).
> - [`vuln-response.md`](vuln-response.md) §5 — *key USE: compromise / loss
>   **revocation*** (the repository-as-revocation-channel retired-key notice when a key
>   must be distrusted; there is no auto-update push channel).
> - **spec §6.2.3** ([`../spec/06-build-test-release.md`](../spec/06-build-test-release.md))
>   — the *`[DECIDED]` key-handling + rotation procedure* (pubkey at
>   `docs/minisign.pub`; the secret as the `MINISIGN_SECRET_KEY` / `MINISIGN_PASSWORD`
>   CI secrets; the announced-signed four-step re-key). This doc **operationalizes**
>   those decisions; it never overrides them.
>
> **Status: living.** Refined during implementation; a change to genesis or custody is
> recorded here first, in the same commit as the change.
> **Conflict order (every layer):**
> **SSOT > spec > security/process docs > plan > code > conversation.**
> When two layers disagree, the higher one wins — never silently reconcile, always
> escalate.

---

## 1. Why genesis & custody need their own policy

The minisign keypair is **not** a routine, rotatable CI secret. Two facts make its
birth and custody load-bearing in a way an API token is not:

- **The whole user-facing trust story rides on it.** With no code-signing and no
  notarization, a user who verifies `minisign -Vm SHA256SUMS -p docs/minisign.pub`
  (the literal recipe build-gates **G39/G44** run at release — spec §6.2.4) is
  trusting that this key was born clean and has only ever been held by the owner. A
  key generated on a compromised host, or whose secret half leaked once, taints every
  release it ever signs.
- **There is no second chance and no push channel.** ConvertIA ships **no updater**
  (spec §7.6.1), so a key that becomes unusable cannot be silently swapped for
  installed users — a re-key is an *announced* event (§4). The secret half lives in
  exactly one online place (a GitHub Environment secret); if that single copy is
  deleted with no backup, the project **permanently loses the ability to sign
  continuations of the same key** — every future release would then force a rotation
  users must notice. Custody is therefore a backup discipline, not a convenience.

Key custody, the backup, and the release-signing material are **owner-held**
([`roles-and-escalation.md`](roles-and-escalation.md) §1); genesis and any loss event
are an **unconditional escalation to the owner** — the autonomous Build-Loop never
generates, holds, or restores this key.

## 2. Genesis — generate off the shared host, never on it

The keypair is generated **air-gapped, off the shared multi-tenant VPS**. The
production CI's self-hosted Linux runner is the **IONOS VPS shared with four other
Ne-IA projects** (spec §6.1.4) — a persistent multi-tenant host that *also* processes
untrusted / adversarial corpus bytes is the textbook host-compromise vector (spec
§6.7.2, security-concept principle 11). A secret key that ever touches that host is
not trustworthy. So:

1. **Generate on a clean, owner-controlled machine** (not the VPS, not any CI
   runner): `minisign -G -p minisign.pub -s minisign.key`, choosing a strong, unique
   passphrase. The generation host holds the secret half air-gapped.
2. **Commit the public half** as `docs/minisign.pub` (restated on the download page,
   spec §6.2.3 / §6.2.4) so anyone can verify; its out-of-band fingerprint anchor is
   asserted at release by **P10.29** (so a pipeline that *could* rewrite the pubkey
   cannot silently substitute a key — build-gates G39 / G44).
3. **Inject the secret half from that off-host generation** straight into the
   `MINISIGN_SECRET_KEY` (the `minisign.key` contents) and `MINISIGN_PASSWORD` secrets
   of the **`release` GitHub Environment** — the approval-gated home that keeps the
   secret structurally unreachable by the autonomous loop until a human approves
   (build-gates **G56**, security-concept §2; provisioned per P0.7.18). The secret is
   **never committed, never placed in the bundle, never written to the shared VPS**.

The release pipeline asserts this custody held — **P10.30** wires this policy into the
release (air-gapped genesis + the §3 backup + the §4 loss-recovery path).

## 3. Custody — an offline, encrypted backup of BOTH halves

The single GitHub-Environment copy is **not** a backup of itself: a deleted or
corrupted secret with no other copy is an unrecoverable signing dead-end (§1). So the
owner keeps an **offline, encrypted backup of BOTH the secret key file AND its
passphrase**, with these properties:

- **Both halves.** The key file alone is useless without the passphrase and vice
  versa — the backup covers **both**, or it is not a backup. (This is the same
  key + passphrase backup [`vuln-response.md`](vuln-response.md) §5 names for the
  loss-survivability of the revocation channel; this doc is where the backup is
  *established*, §5 is where it is *used*.)
- **Off-platform, and the two halves kept apart.** The backup lives off the Ne-IA
  platform (not on the VPS, not in the repo, not only in the GitHub secret), and the
  passphrase is stored **separately** from the encrypted key, so one compromised store
  yields neither a usable key nor a plaintext passphrase.
- **Owner-held, encrypted at rest.** Custody is the owner's
  ([`roles-and-escalation.md`](roles-and-escalation.md) §1); the backup is encrypted
  so a stolen backup medium is not itself a key leak.

## 4. Loss-recovery — the decision path (restore vs rotate)

A loss event forks on a single question: **is the §3 backup intact?**

- **Survivable loss — the backup restores the same key.** The working copy / the
  GitHub-Environment secret was lost or deleted, **but the §3 encrypted backup is
  intact** → restore `MINISIGN_SECRET_KEY` / `MINISIGN_PASSWORD` from the backup into
  the `release` Environment. The **same key continues**; `docs/minisign.pub` is
  unchanged, so all prior releases stay verifiable and no user-visible rotation is
  needed. This is precisely the case the §3 backup exists to guarantee.
- **Unrecoverable loss or compromise — a rotation event.** Both the working copy
  **and** the backup are gone, **or** the secret half is believed leaked → the key
  cannot (or must not) continue, so this is a **rotation**, executed as the *announced,
  signed, retired-key-preserving* re-key of **spec §6.2.3** (commit the new
  `docs/minisign.pub` in a rotation-announcing dedicated commit; retain the old key as
  `docs/minisign-retired.pub`; note the rotation in the first new-key release notes;
  rotate the CI secrets) and surfaced to users through the
  **repository-as-revocation-channel** retired-key notice of
  [`vuln-response.md`](vuln-response.md) §5. A rotation is **never a silent swap** —
  the announced retired-key + release-note trail is exactly the property a
  supply-chain attacker's silent key substitution would lack.

The fork is decided **by the owner** — a loss or a suspected compromise is an
unconditional escalation, never a Build-Loop call.

## 5. Change-control — this doc is L(-1)

This policy is a **security / process doc**, so it is in the **L(-1)
security-critical-file set** (security-concept §2; the machine-readable cage
`scripts/l-neg1-files.toml` matches it via `docs/process/**`). The autonomous
Build-Loop never edits it; an owner-driven change records the `L-neg1-ack: owner`
commit-body trailer that the pre-push gate **G71** audits. Silently weakening the
genesis or custody discipline is exactly the class of edit the cage exists to prevent.

## 6. References

- The key-handling + rotation `[DECIDED]` facts this doc operationalizes (pubkey
  `docs/minisign.pub`; the `MINISIGN_SECRET_KEY` / `MINISIGN_PASSWORD` secrets; the
  four-step announced re-key): spec **§6.2.3** + the verify recipe **§6.2.4**
  ([`../spec/06-build-test-release.md`](../spec/06-build-test-release.md)).
- The key-USE side — compromise / loss **revocation** via the repository retired-key
  channel, and the key + passphrase backup this doc establishes:
  [`vuln-response.md`](vuln-response.md) §5.
- The release-tier gates that run the literal verify recipe + assert the pubkey
  fingerprint anchor: **G39 / G44** ([`../security/build-gates.md`](../security/build-gates.md));
  the pipeline steps that wire this custody in + anchor the pubkey: **P10.30 / P10.29**
  ([`../plan/P10-release.md`](../plan/P10-release.md)).
- The `release` Environment that holds the secret behind a human approval, and the
  self-hosted-runner host-isolation rationale: **G56** + security-concept **§2**
  ([`../security/security-concept.md`](../security/security-concept.md)), spec
  **§6.7.2** ([`../spec/06-build-test-release.md`](../spec/06-build-test-release.md)).
- Who holds the key / who escalates a loss:
  [`roles-and-escalation.md`](roles-and-escalation.md) §1.
- The plan box that authored this doc:
  [`../plan/P0-build-and-security.md`](../plan/P0-build-and-security.md) §P0.7.16.
- SSOT (the no-store / no-cert trust-substitute premise):
  [`../SINGLE-SOURCE-OF-TRUTH.md`](../SINGLE-SOURCE-OF-TRUTH.md).
