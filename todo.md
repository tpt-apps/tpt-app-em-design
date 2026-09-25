# TPT EM Design — Build Todo

> Hub listing: `tptsolutions.co.nz/tools/em-design` (registry slug
> `em-design`, category `engineering`). Spec: `apps.md` §T1-5 in the
> tpt-electrician-nz-2 repo. Free WASM edition: single trace-impedance
> calculation. Paid offline edition ($79 USD, Gumroad): antenna design tools,
> basic 2D field visualization, and a saved project library across designs.
> Solver wraps `tpt-electromagnetics`. UI on `tpt-appfront`.
>
> Hub contract: `wasm-bindgen --target web` glue exposes `mount(container)`
> (called by `WasmAppRunner` after `default()` init). Standalone `trunk serve`
> mounts into the `#tpt-appfront-root` marker div instead — never
> `document.body` — so the two hosts never double-mount.

## Phase 0 — Scaffold & pipeline proof
- [ ] `tpt-appfront init tpt-app-em-design --target dom` (path deps on the tpt-appfront checkout)
- [ ] Restructure scaffold to hub contract: `#[wasm_bindgen] pub fn mount(el)` + `#[wasm_bindgen(start)]` no-op unless `#tpt-appfront-root` present
- [ ] `index.html` carries the `#tpt-appfront-root` marker div
- [ ] Build pipeline script mirroring fea-lite/cutlist-optimizer's (`--target web` glue → web repo `public/apps/em-design/`)
- [ ] Hello-world build loads through the standalone dev page (`trunk serve`)
- [ ] git init + first commit

## Phase 1 — Solver integration (wrap tpt-electromagnetics)
- [ ] Survey `tpt-electromagnetics` for trace-impedance formulas (microstrip/
      stripline) and any antenna-design or field-solve capability it already has
- [ ] Adapter crate `engine/`: trace/antenna/field problem in → impedance,
      antenna dimensions/matching, or field data out as plain structs — no UI
      deps, host-unit-testable
- [ ] Validation cases against known reference formulas (standard microstrip
      impedance calculators) to establish accuracy bounds
- [ ] Wasm payload size check once the sub-crate set is chosen

## Phase 2 — Free WASM UI
- [ ] Single trace-impedance calculator (microstrip/stripline inputs: width,
      height, dielectric constant, copper weight)
- [ ] Free-tier upsell copy pointing at antenna design, field visualization,
      saved project library
- [ ] Browser-verified: mount, calculate, results render correctly

## Phase 3 — Paid desktop edition (cargo feature `pro`)
- [ ] `pro` gates: antenna design tools (length, matching), basic 2D field
      visualization, saved project library across designs
- [ ] Pro UI: antenna design panel, 2D field viewer, project save/load list
- [ ] Desktop shell via `tpt-appfront-webview`; Windows exe build + smoke test
- [ ] Gumroad product from apps.md listing copy ($79), exe upload, URL into
      registry — manual step, listing copy at apps.md §T1-5

## Phase 4 — Ship & measure
- [ ] Registry entry (`engineering` category, `wasm: { entry: '/apps/em-design/em-design.js' }`)
      per the "Adding a finished app to the hub" runbook at the bottom of apps.md
- [ ] Verify hub card/detail/sitemap/runner in dev, then prod after deploy
- [ ] WASM size budget check on the shipped build
