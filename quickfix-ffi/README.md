# QuickFIX FFI

Low level binding for [quickfix](https://github.com/quickfix/quickfix) library.

Check out main [github repository](https://github.com/arthurlm/quickfix-rs/) for more details.

## Performance Build Controls

### io_uring backend (Linux only)

- Enable build-time support: `--features build-with-io-uring`
- Runtime toggle:
  - `QF_IO_URING=1` (default when compiled in)
  - `QF_IO_URING=0` to force legacy `poll`
- Optional runtime tuning:
  - `QF_IO_URING_ENTRIES=<64..8192>` (default `1024`)
  - `QF_IO_URING_MULTISHOT=1|0` (default `1` when supported by liburing headers)

### Sample PGO / AutoFDO

The build script accepts:

- `QUICKFIX_SAMPLE_PROFILE=/path/to/profile` (or `QUICKFIX_AUTOFDO_PROFILE`)
- `QUICKFIX_LTO=off|thin|full`
- `QUICKFIX_PGO_MODE=off|generate|use`
- `QUICKFIX_PGO_DIR=/path/to/pgo-data`
- `QUICKFIX_EXTRA_CFLAGS="..."`
- `QUICKFIX_EXTRA_CXXFLAGS="..."`

Helper wrapper:

```bash
quickfix-ffi/scripts/build_with_sample_pgo.sh \
  --profile /tmp/quickfix.prof \
  -- cargo build -p gateway --release
```

### BOLT post-link

Apply BOLT to a built executable:

```bash
quickfix-ffi/scripts/apply_bolt.sh \
  --binary target/release/gateway \
  --perf-data /tmp/perf.data
```
