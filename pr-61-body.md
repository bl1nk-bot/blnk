## สรุป

PR นี้แก้ Issue #61 ซึ่งทำให้ CI ของ PR #60 ล้มใน Windows job ระหว่าง `cargo check --all-targets` เพราะ dependency กลุ่ม CLI/serialization ถูกวางหลัง target-specific section ของ Cargo manifest ทำให้ test target บน Windows resolve `serde_json` ไม่ได้

## Root cause และการแก้ไข

`serde_json` และ crates ทั่วไปถูกประกาศในส่วนที่ Cargo ตีความเป็น dependency ของ Unix target เนื่องจาก `[target.'cfg(unix)'.dependencies]` อยู่ก่อนกลุ่ม CLI/Utilities การย้าย Unix-only `nix` section ไปไว้หลัง general dependencies ทำให้ `serde_json`, `serde`, `clap` และ utilities เป็น dependencies ของทุก platform ตามที่ source และ test ใช้งานจริง ขณะที่ `nix` ยังคงจำกัดเฉพาะ Unix

## ขอบเขต

- แก้เฉพาะ `Cargo.toml`
- ไม่เปลี่ยน protocol, security policy หรือ runtime behavior
- เป็น stacked PR ต่อจาก PR #60 เพื่อให้ CI ของ threat-model branch ผ่านครบทุก platform

## Validation

ใน sandbox หลัง reset ระหว่างงานไม่สามารถใช้ local Rust validation ได้อย่างต่อเนื่อง เพราะ toolchain และ C linker ถูกล้าง/หายระหว่าง sandbox lifecycle ดังนั้น PR นี้ต้องใช้ GitHub Actions เป็นหลักฐานบังคับ โดยเฉพาะ `Build and test (Windows)` ที่เป็น regression target และ Android compile-only job

ตรวจสอบด้วย `git diff --check` แล้วผ่าน

Closes #61
