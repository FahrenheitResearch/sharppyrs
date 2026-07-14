//! Frequently used meteorological constants (port of `sharppy.sharptab.constants`).

/// Missing data flag used by SHARPpy-style inputs.
pub const MISSING: f64 = -9999.0;
/// R over Cp.
pub const ROCP: f64 = 0.28571426;
/// Zero Celsius in Kelvins.
pub const ZEROCNK: f64 = 273.15;
/// Gravity (m/s^2).
pub const G: f64 = 9.80665;
/// Floating point tolerance.
pub const TOL: f64 = 1e-10;
