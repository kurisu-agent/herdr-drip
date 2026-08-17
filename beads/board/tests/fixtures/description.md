<!--
  A real bead description, near-verbatim from the drift-rust tracker: the
  opening bullets and "Finding A" of dr-0006, plus one ```rust block lifted
  from dr-0116 so the syntect path is exercised. Trimmed only for length.

  It is here because the point of the markdown test is that the content this
  board actually meets survives the renderer: headings, bold, inline code,
  long soft-wrapped prose, a fenced block with a language and one without, and
  a GFM table (which tui-markdown does not turn into a table -- see
  src/markdown.rs -- so the pipes are expected to stay).
-->

- **Partially fixed:** 2026-07-26 — ON MAIN (`2a10ce2`, `5ad8bff`, `fa672cc`, `fd46039`;
  `fix/ticket-sweep` is merged as `ff5d9a0`). Both
  code-provable causes of finding A and the reclaim half of finding B are fixed and
  regression-checked (see "Investigation" below).
- **Then 2026-07-29** — the liveness signal is BUILT and has since **LANDED ON MAIN**
  (`fix/ticket-spike-karts` merged as `85a4122` into `53c2a8b`, both ancestors); see
  "2026-07-29 — the liveness signal, built". Left open because the FIELD failure still
  cannot be attributed without the circuit. **Corrected 2026-08-02 (resync):** this clause
  used to read as unlanded branch work whose "new VM subtests have not run anywhere but this
  branch's gate" — the branch is merged, so the honest wording is landed-but-not-yet-exercised
  on hardware. Read "What remains UNPROVEN" before touching anything.
- **Severity:** a kart silently stops being reachable while lakitu still reports
  `running` — `drift into` fails, and the tap leak accumulates unbounded. **Half stale
  (resync 2026-08-02):** the tap leak is swept, and the liveness note now surfaces in
  `drift into`, `drift kart get` and the desktop — only the `drift karts` roster is still
  silent, which is the one wire field named under "Still not covered".
- **Estimate:** sev med · effort 20 · confidence 88 · ~50k tokens (to the hardware boundary) · as of dbd0dd83 —
  effort 25 → 20 and confidence 80 → 85 on 2026-08-07 (resync). The desk-side remainder is a
  wire field on the `drift karts` roster, and `8fb956c5` is now a landed worked precedent for
  exactly that increment: it added two `Option<_>` fields to `KartSummary`, populated them in

## Finding A — a kart goes unreachable while still "running"

A kart boots, leases, and is reachable (verified: direct ssh in, tools listed).
Some tens of minutes later the host cannot reach it at all, though nothing
reports a failure:

```
$ curl … kart.list                 → {"name":"kilo9","state":"running"}
$ /dev/tcp/10.128.0.2/22           → No route to host
$ ip neigh show 10.128.0.2         → FAILED
$ tcpdump -i kartbr0 arp
  ARP, Request who-has 10.128.0.2 tell 10.128.0.1, length 28   (repeated, NO reply)
```

The L2 path is intact — the guest simply stops answering:

- its tap is UP with carrier and held by the live VM:
  `vk-c8eb51c745c  UP  <BROADCAST,MULTICAST,UP,LOWER_UP>`, and the
  cloud-hypervisor process has `tap=vk-c8eb51c745c`;
- the anti-spoof pin is present:
  `set port_ip4 { "vk-c8eb51c745c" . 10.128.0.2 }`;
- the dnsmasq reservation is present: `02:c8:eb:51:c7:45,10.128.0.2,kilo9`;
- the guest is alive: console shows a completed boot, no panic, no OOM, no
  call trace — just an idle console after the last unit.

### 2026-07-28 — how to tell finding A apart from an [[0003]] guest wedge

These two present IDENTICALLY to an operator — "`drift into` fails, lakitu says `running`" —
and were conflated once already. A kart (`delta7`) reported with exactly that symptom on
2026-07-28 turned out **not** to be finding A. Three probes from the circuit host separate them:

| probe | finding A (this ticket) | 0003 guest wedge |
|---|---|---|
| `ip neigh show <ip>` | `FAILED` | live `lladdr` |
| `ping <ip>` | no route to host | replies, sub-ms |
| `ssh -vv <ip>` | never connects | **connects, then closes pre-banner** |

The `ssh -vv` line is the discriminator, and the failure mode to watch for is
`kex_exchange_identification: Connection closed by remote host` with no `Remote protocol
version` line — the guest's network stack is answering and its sshd is not.


### The shape it left behind

```rust
// rust/crates/turnpike/src/listener.rs:52-60
/// The full upstream response the daemon relays to the kart: status + headers + the
/// buffered body.
pub struct UpstreamResponse { ..., body: Vec<u8> }
```
