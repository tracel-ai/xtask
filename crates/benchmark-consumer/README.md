# xtask benchmark consumer

This non-published crate is the minimal consumer used by the benchmark in the repository README. It always includes
one trivial custom command and exposes three command selections:

| Scenario | Cargo arguments | Enabled base commands |
|----------|-----------------|-----------------------|
| Custom only | `--no-default-features` | None |
| Common four | `--no-default-features --features common` | `build`, `check`, `fix`, `test` |
| All commands | `--no-default-features --features all` | All 21 base commands |

Build each v5 scenario from the workspace root with:

```bash
cargo build --release --package benchmark-consumer --no-default-features
cargo build --release --package benchmark-consumer --no-default-features --features common
cargo build --release --package benchmark-consumer --no-default-features --features all
```
