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

Relative estimation (SAFe style):

```powershell
cargo run -- --input data/tasks.sample.json --budget 7 --relative
```

Raw 1 to 10 scores drift between people and between planning sessions.
`--relative` rewrites each cost-of-delay column against its smallest item,
which becomes 1, and snaps every other item to 1, 2, 3, 5, 8, 13 or 20. The
coarse steps stop the ranking from rewarding false precision, and scores of 0
are shifted so the anchor still lands on 1.

## Test

```powershell
cd forgequeue-rs
cargo test
```
