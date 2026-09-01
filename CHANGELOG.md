# Changelog

ทุกรุ่นด้านล่างเป็นบันทึก release ย้อนหลังตามลำดับที่ PR ถูก merge เข้า `main` แต่ละรุ่นเพิ่ม patch หนึ่งครั้ง (`0.0.1`) และ tag ชี้ merge commit ของ PR นั้นโดยตรง

## [0.2.10] — PR #115
- fix(security): replace short-circuiting PIN length check in orchestration

## [0.2.9] — PR #107
- chore: add just bump recipe, git hooks, and fix peer discovery loop

## [0.2.8] — PR #106
- docs: เพิ่ม demo animation (.gif) และ terminal screen captures (.png) พร้อมปรับปรุง README สื่อสารความสามารถหลักของโครงการให้ชัดเจน
## [0.2.7] — PR #102
- feat(installer): เพิ่ม one-line install & update scripts (`install.ps1`, `update.ps1`, `install.sh`) และปรับปรุง CI/Release workflows ให้ประหยัดโควตา
- docs: อัปเดต `TODO.md` และ `README.md` พร้อมคู่มือการติดตั้งและสถานะ roadmap ล่าสุด

## [0.2.6] — PR #101
- fix(security): ใช้ Sha256 digest constant-time check ใน constant_time_string_eq และส่งผ่าน typed error แทน expect ใน PEM encoding

## [0.2.5] — PR #92
- security(audit): เพิ่ม cargo audit step ใน CI ตรวจสอบ dependency vulnerabilities (closes #86)

## [0.2.4] — PR #93
- perf(bench): เพิ่ม benchmark suite สำหรับ SWSP framing throughput และ cryptographic operations (closes #87)

## [0.2.3] — PR #97
- test(compat): เพิ่ม live interoperability integration test suite vs reference bitbang-cli (closes #85)

## [0.2.2] — PR #91
- feat(nat): รองรับการกำหนด STUN/TURN ICE servers ผ่าน Config และ PeerHandle::with_ice_servers (closes #84)

## [0.2.1] — PR #100
- docs: เพิ่ม `CONTRIBUTING.md` และ `PROTOCOL.md` อธิบายขั้นตอนการมีส่วนร่วมและ SWSP protocol architecture (closes #88)

## [0.2.0] — PR #98
- release: build Linux, Windows, and Android packages from version tags; publish verified `SHA256SUMS` to the matching GitHub Release

## [0.1.37] — PR #75
- feat(web): เพิ่ม loopback browser control surface (`4a01ad8`)

## [0.1.36] — PR #74
- fix(security): ตรวจ PIN และป้องกัน encrypted request replay (`d4a6e9a`)

## [0.1.35] — PR #72
- feat(cli): ทำ remote signaling และ WebRTC orchestration ให้ใช้งานจริง (`3fab3f6`)

## [0.1.34] — PR #59
- ci: establish Linux Windows Android support matrix (`c82a991`)

## [0.1.33] — PR #58
- test(compat): add versioned protocol compatibility baseline (`434f3b4`)

## [0.1.32] — PR #57
- feat(cli): connect MVP commands to authenticated runtime (`0bba4d1`)

## [0.1.31] — PR #56
- feat(proxy): add concrete TCP WebSocket and HTTP streams (`aa4d20f`)

## [0.1.30] — PR #70
- fix(proxy): แก้ CI test เรียก resolved target ไม่ตรง API (`d8b1f2c`)

## [0.1.29] — PR #55
- security(proxy): define deny-by-default proxy policy and ADR (`b62eaaf`)

## [0.1.28] — PR #54
- feat(shell): implement shell stream MVP (`cd48936`)

## [0.1.27] — PR #60
- security: add threat model and runtime hardening audit (`bd2cd1d`)

## [0.1.26] — PR #68
- fix(session): make remote-first shutdown idempotent (`cce647b`)

## [0.1.25] — PR #66
- fix(proxy): จำกัด HTTP response body แบบ incremental (`8fb84e8`)

## [0.1.24] — PR #62
- fix(ci): แก้ Windows dependency scope สำหรับ serde_json (`01095e9`)

## [0.1.23] — PR #64
- test: make Windows shell fixtures portable (`33bb170`)

## [0.1.22] — PR #53
- feat(file): implement sandboxed file-transfer MVP (`62a6c67`)

## [0.1.21] — PR #52
- feat(session): integrate authenticated runtime over peer (`9e71051`)

## [0.1.20] — PR #51
- feat(peer): implement WebRTC peer lifecycle harness (`fb0fd72`)

## [0.1.19] — PR #50
- docs: เพิ่ม Thai Issue Forms และแปล Issues (`edea4da`)

## [0.1.18] — PR #36
- fix(signaling): harden endpoint policy (`4cff39c`)

## [0.1.17] — PR #34
- feat: implement signaling WebSocket transport and local fixture (`71b69c8`)

## [0.1.16] — PR #30
- Add upstream identity PEM compatibility boundary (`4710589`)

## [0.1.15] — PR #32
- Docs: synchronize implementation status and roadmap (`0fddcca`)

## [0.1.14] — PR #12
- feat: add signaling session and stream foundations (`4862c2b`)

## [0.1.13] — PR #26
- fix: harden session and signaling validation (`111b732`)

## [0.1.12] — PR #10
- feat: implement SWSP raw frame codec (`0d4653f`)

## [0.1.11] — PR #25
- fix: classify SWSP length failures correctly (`e288b1e`)

## [0.1.10] — PR #8
- feat: implement identity and pairing primitives (`0f77fbe`)

## [0.1.9] — PR #24
- fix: harden identity and pairing boundaries (`4853e66`)

## [0.1.8] — PR #29
- fix: remove duplicate target dependency tables (`51a2c8a`)

## [0.1.7] — PR #28
- fix: restore shared dependency scope for Linux CI (`c6894ce`)

## [0.1.6] — PR #6
- feat: add runnable Rust foundation (`1476b30`)

## [0.1.5] — PR #22
- fix: reconcile foundation review findings (`c23d4a4`)

## [0.1.4] — PR #14
- build: add deterministic protobuf generation boundary (`f411123`)

## [0.1.3] — PR #4
- docs: reconcile implementation status and protocol decisions (`ac5aa2d`)

## [0.1.2] — PR #3
- [WIP] Set up code coverage reporting in CI workflows (`a80f483`)

## [0.1.1] — PR #2
- feat: scaffold initial foundation and docs (`235fecc`)
