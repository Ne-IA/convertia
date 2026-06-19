# ConvertIA — Release-Pipeline Trust (tag protection · signing provenance · the approval-gated key)

> **What makes a `v*` tag actually mint a *trustworthy* signed release in the
> autonomous direct-to-`main` model.** ConvertIA has no PR and no second reviewer
> (security-concept §2): the only things that turn "CI is green" into an actual
> *block*, and a tag into a *trusted* release, are **invisible GitHub repo config**
> (rulesets, the `release` Environment) and a **cryptographic signing chain** the
> codebase cannot see by itself. This doc is the policy for the three trust planes
> that close that gap, **and the owner-provisioning runbook** for the parts only the
> owner can perform.
>
> **Boundary — where this sits among the trust docs:**
> - **This doc** — the *release-pipeline* trust controls: tag-creation protection,
>   commit/tag signing, and the human-approved, minimally-scoped signing job
>   (build-gates **G56 / G56a / G56b**).
> - [`minisign-key-custody.md`](minisign-key-custody.md) — the *key's* genesis,
>   backup, and loss-recovery (the secret this pipeline injects).
> - [`vuln-response.md`](vuln-response.md) §5 — *revocation* of a distrusted key.
> - **spec §6.7.1 / §6.7.2**
>   ([`../spec/06-build-test-release.md`](../spec/06-build-test-release.md)) — the
>   Lane-A / Lane-B definitions whose trust controls this doc operationalizes; it
>   never overrides them.
>
> **Status: living.** Refined during implementation; a change to a trust plane is
> recorded here first, in the same commit as the change.
> **Conflict order (every layer):**
> **SSOT > spec > security/process docs > plan > code > conversation.**
> When two layers disagree, the higher one wins — never silently reconcile, always
> escalate.

---

## 1. The problem — a red run, an unsigned tag, or an un-approved key must not ship

The single-branch, direct-to-`main`, autonomous-commit model removes the human from
the per-change loop. Three gaps follow, each one this doc closes with a named plane:

- **G56a guards the `main` *branch* ref — it says nothing about *tags*.** The loop (or
  a compromised `GITHUB_TOKEN`, or a stale/forced tag) could create a `v*` tag on
  **any** commit — including one that never passed L4 green — and mint a signed
  artifact. → **Plane 1** (§2).
- **Authorship of a `main` commit / a release tag is unverified.** A leaked
  `GITHUB_TOKEN` could push a commit with a forged `Dual-Review:` trailer + a spoofed
  author, or plant a lightweight tag, that feeds the next legitimate release. →
  **Plane 2** (§3).
- **The one irreversible action — minisigning `SHA256SUMS` — is secret-bearing.**
  Aborting *after* the secret is injected ("abort before read") is not the same as
  the secret **never being injected** without a human's say-so. → **Plane 3** (§4).

Planes 1 and 2 are box **P0.7.17** (`§6.7.1 · G56b G56a`); plane 3 is box **P0.7.18**
(`§6.7.2 · G56`). The **policy** is authored here; the **provisioning** is the owner
action collected in §5.

## 2. Plane 1 — only the owner creates a release tag (G56b leg 1)

**Policy.** A GitHub **tag-protection ruleset on `v*`** restricts release-tag *creation*
to the owner / a protected actor, with a **minimal, enumerated bypass-actor list** —
the **tag-ref sibling** of G56a's branch-ref protection. It is asserted via
`gh api repos/:owner/:repo/rulesets` on **schedule + on tag**, **fail-soft in the P0
bootstrap box, then hard** (build-gates **G56b** leg 1).

**Belt-and-suspenders (already authored in P0.2, fail-closed always).** Even with the
ruleset, the release workflow's **first step** re-checks (G56b leg 2) that the tagged
commit is an **ancestor of `origin/main`** *and* that `main`'s required checks were
**green for that exact SHA**, and **aborts before any secret is read** otherwise. The
ruleset (this plane) and the in-workflow ancestry/green-history assertion (P0.2) are
distinct controls: the ruleset stops the *creation*, the workflow step stops the
*release* if a tag slips onto an unverified commit.

## 3. Plane 2 — the loop signs its own commits and tags (G56b leg 3 + G56a sub-check (g))

**Policy.** The Build-Loop holds an **SSH signing key**; the repo commits a **public SSH
allowed-signers file** at `.github/allowed_signers`, and `git config
gpg.ssh.allowedSignersFile` points at it. Two assertions ride on this one key:

- **Release-tag verification (G56b leg 3).** The loop signs every `v*` tag
  (`git tag -s`); the release workflow's first step runs `git verify-tag` against the
  committed allowed-signers file and **aborts before any secret is read** on failure —
  so a lightweight/unsigned tag planted by a compromised `GITHUB_TOKEN` cannot trigger
  a release. The G56b row's **target** posture is **fail-closed always**, but the
  as-built P0.2.9 assertion is **fail-soft (skip-with-warning) until
  `.github/allowed_signers` lands** — a clean checkout cannot `git verify-tag` before
  that file exists — **then fail-closed**. So leg 3 is genuinely *enforced* only once
  **P0.7.17** is provisioned (the P10.4 `needs: P0.7.17` STOP §5 spells out); until then
  legs 1 + 2 carry the tag trust.
- **Commit authenticity (G56a sub-check (g)).** The **same** key signs the loop's own
  `main` commits (`git config commit.gpgsign true`), so `main`'s ruleset
  **`required_signatures`** knob is *satisfiable* — closing the integrity axis a
  forged `Dual-Review:` trailer + spoofed author would otherwise open. **Fail-soft in
  the P0 bootstrap box (commit-signing is wired mid-P0), then hard** (G56a (g)).

**Distinct from the spec §6.7.1 DCO posture — no conflict.** §6.7.1's
`Signed-off-by` is *requested-not-required text* on **external contributors' commits**
("CI does not hard-block"). Leg 3 is the **loop's own cryptographic signature** on its
**own** release tags + commits — a different artifact, a different actor. They
coexist.

**L(-1).** The committed `.github/allowed_signers` file is in the L(-1)
security-critical-file set (security-concept §2 names "the G56b SSH allowed-signers
file"; the cage `scripts/l-neg1-files.toml` matches it via `.github/**`) — an edit is
a Co-Pilot escalation carrying the `L-neg1-ack: owner` trailer (G71).

## 4. Plane 3 — the one irreversible action is human-approved + minimally scoped (G56)

### 4.1 The `release` GitHub Environment (the human-in-the-loop)

**Policy.** The release secrets (`MINISIGN_SECRET_KEY` / `MINISIGN_PASSWORD` / the
signing-relevant set) live as **Environment** secrets in a **`release` GitHub
Environment** with **required-reviewers + a `v*` deployment-branch/tag policy**; the
signing/release job declares **`environment: release`**, so the secret is **never
injected until the owner approves** — the human-in-the-loop on the one irreversible
action. This turns G56b leg-2's "abort *before reading* the secret" into "secret
**never injected** without approval" — **strictly additive** to leg 2, not a
replacement. Asserted via `gh api repos/:owner/:repo/environments/release`
(`required_reviewers` + the `v*` `deployment_branch_policy` + the `environment:
release` binding), **fail-soft in the P0 bootstrap box, then hard** (build-gates
**G56**; the assertion is wired in P0.2).

### 4.2 The release-job token scope

**Policy.** The release workflow's token is **`contents: write` ONLY**, with
**`id-token: write` only on the release/attestation job**, and it **never runs on a
fork PR**. A G56 jq-over-parsed-YAML sub-assertion enforces that the **workflow-level
`permissions:`** sets `id-token` **absent or `none`** *and* that **`id-token: write`
appears ONLY on the release/attestation job** — the OIDC token is as valuable as a
long-lived secret for impersonation / Sigstore minting, so the scope is closed
explicitly rather than assumed covered by the general `GITHUB_TOKEN`-scope lint (G50).

**Host isolation (cross-reference, not restated).** The secret-bearing signing job
runs on an **ephemeral GitHub-hosted runner under `step-security/harden-runner` (BLOCK
mode)**, host-isolated from the untrusted-corpus VPS leg — the rationale + the G56
self-hosted-label ban live in spec **§6.7.2** + security-concept **§2 / principle 11**
and the **G56** row; this doc points at them.

## 5. The owner-provisioning — completed (the runbook + the STOP backstop)

Both boxes **were `[!extern]`** in the plan until provisioned — the policy is authored,
but the provisioning was an **owner action the loop could not take** (a signing key,
GitHub repo config, the release secrets); **both are now `[x]`** (provisioned in the
owner-present P0 bootstrap session — see the P0.7.17/.18 Delivered notes). Both remain
**STOP-enforced** at the release boundary (the durable backstop, now satisfied), so the
loop cannot mint a release unless each stays provisioned:

- **P0.7.18 — the `release` Environment STOP.** The release box **P10.6** carries
  `needs: P0.7.18`, so per [`../plan/_format.md`](../plan/_format.md) §2/§6 the loop
  **STOPs** rather than minting a release until the `release` Environment is
  provisioned — the secret is structurally unreachable without it.
- **P0.7.17 — the tag-trust STOP.** The release-workflow box **P10.4** carries
  `needs: P0.7.17`, so the loop **STOPs** before standing up the release workflow
  until the SSH signing key is provisioned — because G56b **leg 3** (`verify-tag`) is
  only **fail-closed** once the committed `.github/allowed_signers` exists (it is
  **fail-soft / skip-with-warning** in the P0.2.9 bootstrap window until then, §3).
  Until that STOP releases, **legs 1 + 2** (the `v*` tag-protection ruleset + the
  ancestry/green-history abort) carry the tag trust.

The two STOPs are **symmetric** — together they guarantee no release is minted until
**both** the signing key (tag trust) and the approval-gated secret (key custody) are
in place. The exact owner steps:

**P0.7.17 — tag protection + signing key (planes 1 + 2):**
1. **Generate an SSH signing key** on the build machine, e.g.
   `ssh-keygen -t ed25519 -C "convertia release signing" -f ~/.ssh/convertia_sign`,
   and hand the **public** line (`~/.ssh/convertia_sign.pub`) to the loop so the
   committed `.github/allowed_signers` can be authored with the real key (an L(-1)
   edit under the owner ack). The **private** key never leaves the owner's control.
2. **Wire git signing** on the build machine: `git config gpg.format ssh`;
   `git config user.signingkey ~/.ssh/convertia_sign.pub`;
   `git config commit.gpgsign true`; `git config tag.gpgsign true`;
   `git config gpg.ssh.allowedSignersFile .github/allowed_signers`.
3. **Create the `v*` tag-protection ruleset** (Settings → Rules → Rulesets → new *tag*
   ruleset targeting `v*`, restrict *tag creations* to the owner, bypass-actor list
   minimal) so only the owner can create a release tag.

**P0.7.18 — the `release` Environment + secrets (plane 3):**
4. **Create the `release` Environment** (Settings → Environments → `release`) with
   **required reviewers** (the owner) + a **deployment branch/tag policy** restricting
   it to **`v*`** tags.
5. **Add the secrets** `MINISIGN_SECRET_KEY` + `MINISIGN_PASSWORD` to that Environment
   (the key material is generated per [`minisign-key-custody.md`](minisign-key-custody.md)
   §2 — air-gapped, off the shared VPS).

Each box flipped `[!extern]` → `[x]` when its provisioning landed (P0.7.17: the real
`.github/allowed_signers` committed from the owner's public key + the `v*` and
`required_signatures` rulesets, G56a verified GREEN against the live config; P0.7.18:
the `release` Environment + the two secrets, the minisign keypair roundtrip-verified
against `docs/minisign.pub`).

## 6. Change-control — this doc and `.github/allowed_signers` are L(-1)

This policy is a **security / process doc** (`docs/process/**`) and the allowed-signers
file is `.github/**` — both are in the **L(-1) security-critical-file set**
(security-concept §2; the cage `scripts/l-neg1-files.toml`). The autonomous Build-Loop
never edits either; an owner-driven change records the `L-neg1-ack: owner` trailer the
pre-push gate **G71** audits.

## 7. References

- The trust gates this doc operationalizes — **G56** (runner-host integrity + the
  `release` Environment sub-assertion + the `id-token` scope sub-rule), **G56a** (the
  `main` branch-protection config incl. sub-check (g) `required_signatures`), **G56b**
  (the `v*` release-tag trust gate, legs 1/2/3):
  [`../security/build-gates.md`](../security/build-gates.md).
- The lane definitions + the signing-runner host-isolation: spec **§6.7.1** (Lane A /
  the DCO posture) + **§6.7.2** (Lane B / the signing job)
  ([`../spec/06-build-test-release.md`](../spec/06-build-test-release.md)).
- The L(-1) set (naming the allowed-signers file) + principle 11 (host isolation):
  security-concept **§2** ([`../security/security-concept.md`](../security/security-concept.md)).
- The key the signing job injects (genesis / backup / loss-recovery):
  [`minisign-key-custody.md`](minisign-key-custody.md); the CVE → user + key-revocation
  path: [`vuln-response.md`](vuln-response.md).
- The enforcement halves authored elsewhere: the in-workflow ancestry/green-history +
  `verify-tag` assertion in **P0.2** (the release-workflow skeleton); the consuming
  release pipeline in [`../plan/P10-release.md`](../plan/P10-release.md).
- The plan boxes that authored this doc:
  [`../plan/P0-build-and-security.md`](../plan/P0-build-and-security.md) §P0.7.17 / §P0.7.18.
- Who provisions / who escalates: [`roles-and-escalation.md`](roles-and-escalation.md) §1.
- SSOT (the no-store / no-cert trust-substitute premise):
  [`../SINGLE-SOURCE-OF-TRUTH.md`](../SINGLE-SOURCE-OF-TRUTH.md).
