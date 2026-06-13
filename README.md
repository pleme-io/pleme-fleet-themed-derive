# pleme-fleet-themed-derive

`#[derive(FleetThemed)]` — mechanize the `ishou_tokens::FleetThemedConfig`
`from_fleet(&FleetDefaults) -> Self` constructor. Convergence by
construction across the pleme-io fleet: touch `FleetDefaults` once,
every app's prescribed visual tier re-derives on the next compile.

[![crates.io](https://img.shields.io/crates/v/pleme-fleet-themed-derive.svg)](https://crates.io/crates/pleme-fleet-themed-derive)

Part of the pleme-io ★★ EMITTER SUBSTRATE: every recurring impl block is
a derive. The flagship hand-written impl (`mado@556b685` `src/config.rs`)
was the first production `FleetThemedConfig` impl in the fleet; this
derive turns its mechanizable common part into one line.

## Install

```toml
[dependencies]
ishou-tokens = "*"               # the FleetThemedConfig trait + FleetDefaults
pleme-fleet-themed-derive = "*"
```

## Usage

```rust
use pleme_fleet_themed_derive::FleetThemed;

#[derive(FleetThemed)]
#[fleet(base = "MyAppConfig::bare", finalize = post_fleet)]
struct MyAppConfig {
    // by-name: field name matches a FleetDefaults field → fd.theme.clone()
    #[fleet]                       theme: String,
    // explicit source field
    #[fleet(font_family)]          family: String,
    // Copy scalar (no .clone())
    #[fleet(font_size, copy)]      size: f32,
    #[fleet(line_height, copy)]    line_height: f32,
    // transform via fn(&FleetDefaults) -> T escape hatch (nested structs,
    // per-OS splits, name→enum maps live here)
    #[fleet(with = map_cursor)]    cursor: CursorStyle,
    // literal default for a field with no fleet analogue
    #[fleet(default = 80u32)]      cols: u32,
    // skip / unannotated → taken from the base ctor via ..base()
    #[fleet(skip)]                 profiles: Profiles,
    shell: ShellConfig,            // unannotated == skip
}
```

### Field attribute grammar — `#[fleet(...)]`

| form                       | emits                                  |
|----------------------------|----------------------------------------|
| `#[fleet]`                 | `field: fd.field.clone()`              |
| `#[fleet(<src>)]`          | `field: fd.<src>.clone()`              |
| `#[fleet(<src>, copy)]`    | `field: fd.<src>` (no clone — scalars) |
| `#[fleet(with = path)]`    | `field: path(fd)` (escape hatch)       |
| `#[fleet(default = expr)]` | `field: expr`                          |
| `#[fleet(skip)]` / none    | taken from the base ctor (struct-update)|

### Struct attribute grammar — `#[fleet(...)]` on the item

| form                        | effect                                                  |
|-----------------------------|---------------------------------------------------------|
| `#[fleet(base = "path")]`   | base ctor for skipped fields (default `Default::default`)|
| `#[fleet(finalize = path)]` | call `path(&mut self, fd)` after the struct is built     |
| `#[fleet(copy_default)]`    | scalar fleet fields copied by default (no `.clone()`)    |

The escape hatches (`with`, `finalize`, `base`) keep the app's
genuinely-unique tail — nested theme-surface mapping, per-OS decoration
splits, value transforms — small and typed while the flat
`FleetDefaults → field` assignments are mechanized.

## Wiring the convergence Guard

Pair with `ishou_tokens::convergence::Guard` so the derived impl is
pinned at test time:

```rust
#[test]
fn fleet_convergence() {
    ishou_tokens::convergence::Guard::for_app("myapp")
        .expect_font_family(MyAppConfig::default().family.as_str())
        .expect_font_size(MyAppConfig::default().size)
        .expect_theme(/* resolved FleetTheme */)
        .run();
}
```

## Generation discipline

Emits source through `quote!{}` + `syn` (★★ TYPED EMISSION — no
`std::format!`). Token-stability tests assert on the emitted token
stream via `tatara-rust-snapshot`, never on formatted output.

## License

MIT © pleme-io
