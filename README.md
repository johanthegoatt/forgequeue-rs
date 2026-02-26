# ForgeQueue RS

Rust CLI/library for high-throughput prioritization using WSJF-style scoring.

## Features

- Rank tasks by weighted shortest-job-first
- Select delivery batch under effort budget
- Fast deterministic prioritization logic with tests

## Run

```powershell
cd forgequeue-rs
cargo run -- --input data/tasks.sample.json --budget 7
```

JSON mode:

```powershell
cd forgequeue-rs
cargo run -- --input data/tasks.sample.json --budget 7 --json
```

Filter by score and top-N:

```powershell
cd forgequeue-rs
cargo run -- --input data/tasks.sample.json --budget 7 --min-score 6 --top 2
```

## Test

```powershell
cd forgequeue-rs
cargo test
```
