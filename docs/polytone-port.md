# Polytone port: status and next steps

Polytone is Java code, so it cannot be a `.hmod`. Horizon reads the Polytone pack format natively in
`crates/horizon-polytone`, written clean-room from the public wiki
(https://github.com/MehVahdJukaar/polytone/wiki) and from packs. Do not read or decompile the mod.
Work only in this repository.

## Done

- Expressions, targets (id, `#tag`, regex), priority resolution, attribute layers.
- Colormaps (`polytone/colormaps`), `colors.json` biome colours, fluid modifiers.
- Applied in game through `crates/horizon-client-render/src/biome.rs`:
  - grass, foliage, dry foliage and water colour from `colors.json` and biome-only colormaps
    (a biome's own colour still wins, water is replaced everywhere);
  - water tint from a fluid modifier whose colormap depends only on the biome;
  - sky, fog and water fog colour from `biome_modifiers` (`biome_attributes.rs`): plain colour or
    number override layers, `base` then `post_process`, highest priority wins.
- Read but not applied: block, item and fluid modifier effects, `sprites`, `creative_tabs`,
  `particles`, `pack` config, dimension modifiers, `colors.json` custom colours.

## Next, in rough order

1. Block modifier effects (tint, render type, offset) through the mesher's tint hook.
2. Item modifier effects (tint, rarity and name colour).
3. Position-dependent colormaps (`POS_X`, `POS_Z`, ...), now logged as "isn't applied yet".
4. Refused colormap features: `color_modifier`, `biome_id`, `legacy_temperature`, `grid`, `damage`.
5. Biome layers that modify (`add`, `multiply`, `alpha_blend`...), `REMOVE`, colormap and expression
   arguments; dimension modifiers and weather groups; fog distances and cloud colour.
6. Wire `sprites`, `creative_tabs`, `particles`, `colors.json` custom colours.

## Rules

- Tests: `cargo +stable test -p horizon-polytone` and
  `cargo +stable test -p horizon-client-render --lib biome` (the pinned 1.95.0 toolchain may not
  download here; stable's clippy flags unrelated `incompatible_msrv` and `unexpected_cfgs`).
- Pull before starting, push to `android` when done; small commits.
