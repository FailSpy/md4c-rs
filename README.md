# md4c-rs

Rust crates built on [MD4C](https://github.com/mity/md4c), a fast CommonMark-compliant Markdown parser.

## Crates

| Crate | Description |
|-------|-------------|
| [md4c-rs](md4c-rs/) | Safe Rust bindings for MD4C |
| [ratatui-md](ratatui-md/) | Markdown rendering widget for ratatui TUIs |

## Upstream

The `upstream/` directory is a git submodule tracking [mity/md4c](https://github.com/mity/md4c). The C sources are vendored into `md4c-rs/vendor/` for crates.io publishing.

To sync vendored sources with upstream:

```bash
git submodule update --remote upstream
cp upstream/src/md4c.c upstream/src/md4c.h \
   upstream/src/md4c-html.c upstream/src/md4c-html.h \
   upstream/src/entity.c upstream/src/entity.h \
   md4c-rs/vendor/
```

## License

MIT — same as MD4C.
