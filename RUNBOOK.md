# ForgeQueue RS Runbook

## Purpose

Prioritize and batch engineering tasks using WSJF scoring under a fixed effort budget.

## Setup

```powershell
cd forgequeue-rs
cargo build
```

## Local Demo

```powershell
cd forgequeue-rs
cargo run -- --input data/tasks.sample.json --budget 7
```

## JSON Output Mode

```powershell
cd forgequeue-rs
cargo run -- --input data/tasks.sample.json --budget 7 --json
```

## Verification

```powershell
cd forgequeue-rs
cargo test
```
