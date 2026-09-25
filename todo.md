# TPT EM Design — Build Todo

> Hub listing: `tptsolutions.co.nz/tools/em-design` (registry slug
> `em-design`, category `engineering`). Spec: `1apps.md` §T1-5 in the
> tpt-electrician-nz-2 repo. Free WASM edition: single trace-impedance
> calculation. Paid offline edition ($149 USD, Gumroad): antenna design tools,
> basic 2D field visualization, and a saved project library across designs.
> Solver: self-contained `engine/` crate — `tpt-electromagnetics` was
> surveyed (2026-09) and has no implemented crates, and its spec defers
> PCB/antenna calculators to app code, so the formulas live here
> (Hammerstad–Jensen microstrip, conformal-mapping stripline, textbook
> antenna/matching closed forms, SOR Laplace field solve), host-testable.
> UI on `tpt-appfront`.
>
> Hub contract: `wasm-bindgen --target web` glue exposes `mount(container)`
> (called by `WasmAppRunner` after `default()` init). Standalone `trunk serve`
> mounts into the `#tpt-appfront-root` marker div instead — never
> `document.body` — so the two hosts never double-mount.

## Phase 0 — Scaffold & pipeline proof
- [x] Scaffold to hub contract: `#[wasm_bindgen] pub fn mount(el)` +
      `#[wasm_bindgen(start)]` no-op unless `#tpt-appfront-root` present
- [x] `index.html` carries the `#tpt-appfront-root` marker div
- [x] Build pipeline `build.ps1`/`build.sh` mirroring fea-lite/cutlist-optimizer's
      (`--target web` glue → web repo `public/apps/em-design/`; `-Pro` builds the
      trunk bundle + desktop exe and never touches the free bundle)
- [x] Hello-world build loads through the standalone dev page (`trunk serve`)
- [x] git init + first commit

## Phase 1 — Solver engine
- [x] Survey `tpt-electromagnetics`: empty workspace, no crates — formulas
      implemented directly in `engine/` instead (see header note)
- [x] Adapter crate `engine/`: trace/antenna/field problem in → impedance,
      antenna dimensions/matching, or field data out as plain structs — no UI
      deps, host-unit-testable
- [x] Validation cases: 30 tests (free 14 / pro 30) — closed-form anchors
      cross-checked with an independent Python evaluation, model identities,
      and a field-solver-vs-closed-form Z0 cross-check (≈1% on test grids)
- [x] Wasm payload size check (see Phase 4)

## Phase 2 — Free WASM UI
- [x] Single trace-impedance calculator (microstrip/stripline inputs: width,
      height/spacing, dielectric constant, copper weight; εeff, delay, C′, L′,
      loss estimate, validity notes)
- [x] Free-tier upsell copy pointing at antenna design, field visualization,
      saved project library
- [x] Browser-verified: mount via `#tpt-appfront-root`, Calculate, samples,
      stackup toggle — Z0 49.02 Ω matches the independent reference exactly

## Phase 3 — Paid desktop edition (cargo feature `pro`)
- [x] `pro` gates: antenna design tools (dipole/monopole/patch + quarter-wave/
      L/single-stub matching), 2D field visualization (SOR Laplace +
      Gauss-law capacitance), radiation-pattern cuts, saved project library
      (localStorage, mixed trace/antenna/matching projects)
- [x] Pro UI: antenna panel, pattern canvas, 2D field viewer canvas, project
      save/load/delete list — browser-verified (patch L/W/inset match Balanis
      hand-calc; field Z0 within 0.9% of closed form; library survives reload)
- [x] Desktop shell via `tpt-appfront-webview` (embeds `../dist`, single-file
      exe with temp-dir extraction); Windows exe build + smoke test
- [ ] Gumroad product from 1apps.md listing copy ($149), exe upload, URL into
      registry — manual step, listing copy at `GUMROAD.md` (apps.md §T1-5)

## Phase 4 — Ship & measure
- [x] Registry entry (`engineering` category, `wasm: { entry: '/apps/em-design/em-design.js' }`)
      added to `tpt-apps-registry.ts` (status `coming-soon` until Gumroad exists)
- [ ] Verify hub card/detail/sitemap/runner in dev, then prod after deploy
- [x] WASM size budget check on the shipped build
