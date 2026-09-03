# 步骤6：测试验证（v0.3.0）

## 检查项

| # | 检查项 | 命令 | 结果 |
|---|--------|------|------|
| 1 | 编译检查 | `cargo check --locked` | ✅ 无 error（耗时 0.05s） |
| 2 | 测试 | `cargo test` | ✅ 7/7 passed（0 ignored, 0 measured） |
| 3 | Lint 检查 | `cargo clippy -- -D warnings` | ✅ 无 error（0 warnings） |

## 测试明细

```
running 7 tests
test diag::tests::test_parse_gc_line_insufficient_fields ... ok
test diag::tests::test_parse_gc_line ... ok
test diag::tests::test_parse_threads_output_simple ... ok
test diag::tests::test_parse_gc_line_with_pid ... ok
test diag::tests::test_parse_threads_output_with_deadlock ... ok
test diag::tests::test_thread_state_from_jcmd ... ok
test diag::tests::test_thread_state_symbols ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 覆盖率

AGENTS.md 质量门禁未定义覆盖率阈值，跳过。

## 结论

✅ 三项检查全部通过，门禁达标。
