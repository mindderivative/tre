//! M23 Phase 1 (§1, §3): a small, real, curated set of genuine
//! Material Symbols icons -- ARCHITECTURE.md §1 Locked Decisions' own
//! "MD3's own icon set embedded as `kurbo::BezPath` data at build
//! time" design. Every `d=` string below is the real, verbatim SVG
//! path data from Google's own icon font CDN
//! (`https://fonts.gstatic.com/s/i/short-term/release/
//! materialsymbolsoutlined/<name>/default/24px.svg`, the "outlined"
//! style at the default weight) -- fetched directly, not invented or
//! approximated. Every one shares the identical real `viewBox="0 -960
//! 960 960"` (`engine_core::ICON_VIEWBOX_SIZE`), confirmed across all
//! eight before relying on it as a fixed constant.
//!
//! Deliberately a small starter set, not the full multi-thousand-icon
//! Material Symbols library -- additive to grow later exactly the way
//! `engine_core::canvas::DrawCommand`'s own real variants have only
//! ever grown when a real need asked for more (this module's own
//! `PLAN.md` has the full reasoning).

/// The real, verbatim `d=` path data for each curated icon, parseable
/// directly via `peniko::kurbo::BezPath::from_svg` (already pinned,
/// zero new dependency -- confirmed via direct source read before this
/// module was written).
const ICONS: &[(&str, &str)] = &[
    (
        "home",
        "M240-200h120v-240h240v240h120v-360L480-740 240-560v360Zm-80 80v-480l320-240 320 240v480H520v-240h-80v240H160Zm320-350Z",
    ),
    (
        "search",
        "M784-120 532-372q-30 24-69 38t-83 14q-109 0-184.5-75.5T120-580q0-109 75.5-184.5T380-840q109 0 184.5 75.5T640-580q0 44-14 83t-38 69l252 252-56 56ZM380-400q75 0 127.5-52.5T560-580q0-75-52.5-127.5T380-760q-75 0-127.5 52.5T200-580q0 75 52.5 127.5T380-400Z",
    ),
    (
        "menu",
        "M120-240v-80h720v80H120Zm0-200v-80h720v80H120Zm0-200v-80h720v80H120Z",
    ),
    (
        "close",
        "m256-200-56-56 224-224-224-224 56-56 224 224 224-224 56 56-224 224 224 224-56 56-224-224-224 224Z",
    ),
    (
        "check",
        "M382-240 154-468l57-57 171 171 367-367 57 57-424 424Z",
    ),
    (
        "arrow_back",
        "m313-440 224 224-57 56-320-320 320-320 57 56-224 224h487v80H313Z",
    ),
    (
        "add",
        "M440-440H200v-80h240v-240h80v240h240v80H520v240h-80v-240Z",
    ),
    (
        "settings",
        "m370-80-16-128q-13-5-24.5-12T307-235l-119 50L78-375l103-78q-1-7-1-13.5v-27q0-6.5 1-13.5L78-585l110-190 119 50q11-8 23-15t24-12l16-128h220l16 128q13 5 24.5 12t22.5 15l119-50 110 190-103 78q1 7 1 13.5v27q0 6.5-2 13.5l103 78-110 190-118-50q-11 8-23 15t-24 12L590-80H370Zm70-80h79l14-106q31-8 57.5-23.5T639-327l99 41 39-68-86-65q5-14 7-29.5t2-31.5q0-16-2-31.5t-7-29.5l86-65-39-68-99 42q-22-23-48.5-38.5T533-694l-13-106h-79l-14 106q-31 8-57.5 23.5T321-633l-99-41-39 68 86 64q-5 15-7 30t-2 32q0 16 2 31t7 30l-86 65 39 68 99-42q22 23 48.5 38.5T427-266l13 106Zm42-180q58 0 99-41t41-99q0-58-41-99t-99-41q-59 0-99.5 41T342-480q0 58 40.5 99t99.5 41Zm-2-140Z",
    ),
    // M30 Phase 6 Step 2 (§1, §3): a real, confirmed need -- `Accordion`
    // (no official M3 component page; grounded in the Lists
    // guideline's own "expand and collapse" text) needs a real
    // expand/collapse chevron for its header. Fetched directly from
    // the identical real CDN this module's own doc comment already
    // cites, not invented or approximated -- the ninth curated icon,
    // growing this set exactly the way its own doc comment says it
    // will, "when a real need asks for more."
    (
        "expand_more",
        "M480-345 240-585l56-56 184 184 184-184 56 56-240 240Z",
    ),
];

/// The real, verbatim `d=` SVG path data for a curated icon `name`, or
/// `None` if it isn't one of the icons this project currently curates
/// (this module's own `PLAN.md`: additive, not exhaustive).
pub fn path_for(name: &str) -> Option<&'static str> {
    ICONS
        .iter()
        .find(|(icon_name, _)| *icon_name == name)
        .map(|(_, path)| *path)
}

/// Every curated icon's own real name, in the same order `path_for`
/// searches them -- `engine-py`'s own real "unknown icon name" error
/// message lists these, and this module's own tests iterate them.
pub fn names() -> impl Iterator<Item = &'static str> {
    ICONS.iter().map(|(name, _)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curated_icon_parses_as_a_real_bezpath() {
        for name in names() {
            let d = path_for(name).unwrap_or_else(|| panic!("{name} must have real path data"));
            peniko::kurbo::BezPath::from_svg(d)
                .unwrap_or_else(|e| panic!("{name}'s own real path data must parse: {e}"));
        }
    }

    #[test]
    fn an_unknown_icon_name_returns_none() {
        assert_eq!(path_for("not_a_real_icon"), None);
    }
}
