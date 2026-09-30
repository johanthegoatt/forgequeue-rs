# Changelog

## 0.3.0 - 2026-10-01

- Added `--relative`: rescales business value, time criticality and risk reduction the way SAFe runs WSJF, anchoring the smallest item in each column at 1 and snapping the rest to the modified Fibonacci scale (1, 2, 3, 5, 8, 13, 20). Effort stays in real units so `--budget` keeps its meaning.

## 0.2.0 - 2026-02-25

- Added JSON CLI input/output support (`--input`, `--budget`, `--json`).
- Added richer batch planning metadata (`budget`, `used_effort`, `skipped_tasks`).
- Hardened prioritization behavior with effort normalization and deterministic tie-breaking tests.
- Added `.gitignore` and sample task data.

## 0.1.0 - 2026-02-23

- Initial WSJF prioritization library and demo CLI.
