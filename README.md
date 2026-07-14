# sharppyrs

The SPC-style **Skew-T sounding plot** from
[SHARPpy-Reimagined-vRust](https://github.com/FahrenheitResearch/SHARPpy-Reimagined-vRust)
(built on [SHARPpy](https://github.com/sharppy/SHARPpy)), ported to
**Rust / [egui](https://github.com/emilk/egui)** as an embeddable widget.

Faithful to the original render: log-p/skewed-T background grid, temperature /
dewpoint / wetbulb / virtual-temperature traces, lifted parcel trace with
orange CAPE + blue CIN buoyancy fill, downdraft parcel trace, LCL/LFC/EL
markers, 0/-20/-30 °C levels, effective inflow layer bracket with ESRH, max
2-6 km lapse-rate bracket, HGZ band, height markers, omega meter, and
speed-colored wind barbs — with the bundled Space Grotesk face.

The full `sharptab` numerical core it needs is ported too (Wobus moist
adiabats, `parcelx` parcel lifting, effective inflow layer, Bunkers storm
motion, helicity, DCAPE) and validated against the Python implementation's
output (see `tests/golden.rs`).

## Usage

```rust
// once at startup (optional, for the original's Space Grotesk look):
sharppyrs::install_fonts(&cc.egui_ctx);

// build a profile from raw sounding data (surface upward):
let profile = sharppyrs::Profile::new(sharppyrs::SoundingData {
    pres,               // hPa
    hght,               // m MSL
    tmpc, dwpc,         // °C
    wdir, wspd,         // deg, kts
    omeg: Some(omeg),   // Pa/s (optional; enables the omega meter)
    latitude: Some(36.7),
    missing: None,      // defaults to -9999.0
}).unwrap();

// in your egui UI:
ui.add(
    sharppyrs::SkewT::new(&profile)
        .title("HRRR 2026-06-25 06z F018  Valid: Fri 2026-06-26 00z")
        .parcel(sharppyrs::ParcelType::MostUnstable) // default, like the original
        .style(sharppyrs::SkewTStyle::space_grotesk()),
);
```

`Profile::new` does all the heavy lifting once (parcels, effective layer,
storm motion, DCAPE); the widget itself just paints, so it is cheap to draw
every frame. Colors are configurable through `SkewTStyle`.

## Demo

```
cargo run --example demo
```

renders the bundled example HRRR sounding.

## Tests

- `cargo test --test golden` — numerical port vs. SHARPpy Python golden data
  (`testdata/golden.json`).
- `cargo test --test snapshot` — headless wgpu render to
  `target/skewt_snapshot.png`.

## License & attribution

BSD-3-Clause, like SHARPpy and SHARPpy Reimagined; this crate is a derived
port of their rendering and `sharptab` algorithms. Space Grotesk is bundled
under the SIL Open Font License 1.1 (`assets/fonts/OFL.txt`).
