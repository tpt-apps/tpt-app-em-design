# Gumroad listing

Everything needed to create/update the Gumroad product once this app is
built. Source copy is `apps.md` §T1-5 in the `tpt-electrician-nz-2` repo — if
that changes, update here too (and vice versa).

## Step-by-step: publish this listing

1. Build the Pro edition (`./build.ps1 -Pro`, once the build pipeline exists
   — see `todo.md` and mirror `tpt-app-fea-lite`/`tpt-app-cutlist-optimizer`'s
   pattern) — produces the packaged exe/zip in `release/`.
2. Log in to Gumroad → **New product** → **Digital product**.
3. Set the product name, price, and upload the packaged file (all below).
4. Paste the short description, bullets, and full description (below) into
   their respective fields.
5. Set category/tags (below) and add a cover image (see note below).
6. Publish, then copy the resulting product URL (`https://gum.co/...`).
7. Paste that URL into the `em-design` entry's `gumroadUrl` field in
   `tpt-electrician-nz-2\src\lib\tpt-apps-registry.ts`, and set `price: '$149'`
   on the same entry.
8. Flip that entry's `status` to `'live'` once both the free demo and this
   Gumroad listing exist.
9. Commit and push the registry repo.

## File to upload

`target\release\tpt-em-design-pro.exe` — produced by `./build.ps1 -Pro`.
The exe is genuinely standalone: the web bundle is embedded at compile time
and extracted to the OS temp dir on first launch, so there is no separate
`dist/` folder to ship or lose. Smoke-tested: launches, extracts its bundle,
and renders in WebView2.

## Product name

```
TPT EM Design — PCB & Antenna Calculations Without an HFSS License
```

## Price

**$149 USD, one-time purchase** (no subscription).

## Short description / summary field

```
Trace impedance, antenna design, and basic field visualization for everyday PCB and RF work — built for electronics engineers who don't need a full 3D EM solver for this.
```

## Bullets / feature list

```
PCB trace impedance calculator
Antenna design tools
2D field visualization
Saved project library
One-time purchase
```

## Full description (long-form field)

```
TPT EM Design covers PCB trace impedance calculation, basic antenna design
(length, matching), and simple field visualization for everyday electronics
design decisions that don't need a full 3D EM solver.

Who it's for: PCB designers, RF hobbyists, and small electronics product
companies needing these calculations without an HFSS or CST license — tens
of thousands per year for a job this doesn't require. A professional
engineering audience, distinct from electricians doing installation work.

Pro edition (this download):
- Antenna design tools
- Basic 2D field visualization
- Saved project library across designs

Try the free single trace-impedance calculation first, right in your
browser, no install: tptsolutions.co.nz/tools/em-design

Requirements: Windows 10 (20H2+) or Windows 11. Uses the WebView2 runtime,
preinstalled on both — nothing extra to download.
```

## Category / tags

- Category: **Software > Design & Engineering** (or Gumroad's closest equivalent)
- Tags: `pcb-design`, `rf-engineering`, `antenna-design`, `electromagnetics`,
  `engineering-software`, `windows`

## Cover image / thumbnail

Recommended once listing: a screenshot of the 2D field viewer on a solved
microstrip cross-section (potential heatmap + equipotential lines), or the
radiation-pattern polar plot beside the trace result. Both render in the Pro
build; no stock/AI art.

## After publishing

1. Copy the resulting product URL (`https://gum.co/...`).
2. Paste it into the `gumroadUrl` field of the `em-design` entry in
   `tpt-electrician-nz-2\src\lib\tpt-apps-registry.ts`, and set `price: '$149'`.
3. Set `status` to `'live'`.
4. Commit and push the registry repo.
