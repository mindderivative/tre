//! 0.5.7 (#198): a layer surface's settings and the rules the protocol puts on them.
//!
//! Pure data and validation, with no Wayland in it, so the rules are checked (and
//! the Python errors raised) on every platform and without a compositor. The
//! `zwlr_layer_shell_v1` protocol makes some combinations a protocol error that
//! kills the client's connection (`invalid_size`, `invalid_anchor`,
//! `invalid_exclusive_edge`, ...); [`LayerConfig::validate`] refuses them first, with
//! the key that is wrong, so the app sees an error it can read.

use std::fmt;

/// The stacking layer: what the surface sits above and below.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerKind {
    Background,
    Bottom,
    Top,
    Overlay,
}

impl LayerKind {
    pub const NAMES: [&'static str; 4] = ["background", "bottom", "top", "overlay"];

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "background" => Self::Background,
            "bottom" => Self::Bottom,
            "top" => Self::Top,
            "overlay" => Self::Overlay,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

/// How the surface takes the keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardMode {
    /// Never: it gets no key events.
    None,
    /// Whenever the user focuses it (a click, say).
    OnDemand,
    /// At once, and no other surface gets keys until it lets go.
    Exclusive,
}

impl KeyboardMode {
    pub const NAMES: [&'static str; 3] = ["none", "on_demand", "exclusive"];

    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "none" => Self::None,
            "on_demand" => Self::OnDemand,
            "exclusive" => Self::Exclusive,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

/// The screen edges a surface is anchored to, as the protocol's bit field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Anchors(pub u8);

impl Anchors {
    pub const TOP: u8 = 1;
    pub const BOTTOM: u8 = 2;
    pub const LEFT: u8 = 4;
    pub const RIGHT: u8 = 8;
    pub const NAMES: [&'static str; 4] = ["top", "bottom", "left", "right"];

    pub fn edge(name: &str) -> Option<u8> {
        Some(match name {
            "top" => Self::TOP,
            "bottom" => Self::BOTTOM,
            "left" => Self::LEFT,
            "right" => Self::RIGHT,
            _ => return None,
        })
    }

    pub fn contains(self, edge: u8) -> bool {
        self.0 & edge == edge
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The edge names set, in the order top, bottom, left, right.
    pub fn names(self) -> Vec<&'static str> {
        [Self::TOP, Self::BOTTOM, Self::LEFT, Self::RIGHT]
            .into_iter()
            .zip(Self::NAMES)
            .filter(|(bit, _)| self.contains(*bit))
            .map(|(_, name)| name)
            .collect()
    }

    /// Whether the protocol gives a positive exclusive zone to this anchor set: one
    /// edge alone, or one edge with both edges across it (a bar anchored top, left and
    /// right). Any other set, a positive zone is treated as 0.
    pub fn can_reserve(self) -> bool {
        let Anchors(bits) = self;
        let horizontal = Self::LEFT | Self::RIGHT;
        let vertical = Self::TOP | Self::BOTTOM;
        matches!(bits, Self::TOP | Self::BOTTOM | Self::LEFT | Self::RIGHT)
            || bits == Self::TOP | horizontal
            || bits == Self::BOTTOM | horizontal
            || bits == Self::LEFT | vertical
            || bits == Self::RIGHT | vertical
    }
}

/// Everything that makes a layer surface what it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerConfig {
    /// What the surface is for (`"tesserae-shell.bar"`): the protocol's namespace.
    pub namespace: String,
    pub layer: LayerKind,
    pub anchor: Anchors,
    /// `(width, height)` in logical pixels; 0 on an axis stretches between the two
    /// edges anchored on it.
    pub size: (u32, u32),
    /// Pixels to reserve along the anchored edge; 0 reserves none; -1 ignores other
    /// surfaces' zones.
    pub exclusive_zone: i32,
    /// For a surface anchored to a corner: which edge its positive zone is for.
    pub exclusive_edge: Option<u8>,
    /// `(top, right, bottom, left)`, in logical pixels. Negative is accepted.
    pub margin: (i32, i32, i32, i32),
    pub keyboard: KeyboardMode,
    /// A connector name (`"DP-1"`); `None` lets the compositor choose.
    pub output: Option<String>,
}

impl Default for LayerConfig {
    fn default() -> Self {
        Self {
            namespace: String::new(),
            layer: LayerKind::Top,
            anchor: Anchors(0),
            size: (0, 0),
            exclusive_zone: 0,
            exclusive_edge: None,
            margin: (0, 0, 0, 0),
            keyboard: KeyboardMode::None,
            output: None,
        }
    }
}

/// Why a configuration was refused: the Python-level key, and what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError {
    pub key: &'static str,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.message)
    }
}

impl std::error::Error for ConfigError {}

/// Something the protocol accepts but that probably is not what was meant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigWarning {
    pub key: &'static str,
    pub message: String,
}

fn error(key: &'static str, message: impl Into<String>) -> ConfigError {
    ConfigError {
        key,
        message: message.into(),
    }
}

impl LayerConfig {
    /// Refuses what the protocol would answer with a fatal error, naming the key; for
    /// what it accepts but ignores, returns a warning.
    pub fn validate(&self) -> Result<Vec<ConfigWarning>, ConfigError> {
        if self.namespace.is_empty() {
            return Err(error(
                "namespace",
                "must not be empty: it says what the surface is for",
            ));
        }
        if self.namespace.contains('\0') {
            return Err(error("namespace", "must not contain a NUL character"));
        }
        let (width, height) = self.size;
        if width == 0
            && !(self.anchor.contains(Anchors::LEFT) && self.anchor.contains(Anchors::RIGHT))
        {
            return Err(error(
                "size",
                "a width of 0 stretches between the left and right edges, so `anchor` must \
                 include both \"left\" and \"right\"",
            ));
        }
        if height == 0
            && !(self.anchor.contains(Anchors::TOP) && self.anchor.contains(Anchors::BOTTOM))
        {
            return Err(error(
                "size",
                "a height of 0 stretches between the top and bottom edges, so `anchor` must \
                 include both \"top\" and \"bottom\"",
            ));
        }
        if let Some(edge) = self.exclusive_edge {
            if !self.anchor.contains(edge) {
                return Err(error(
                    "exclusive_edge",
                    "must be one of the edges in `anchor`",
                ));
            }
        }
        if self.exclusive_zone < -1 {
            return Err(error(
                "exclusive_zone",
                "must be a number of pixels, 0, or -1 (ignore other surfaces' zones)",
            ));
        }
        let mut warnings = Vec::new();
        if self.exclusive_zone > 0 && self.exclusive_edge.is_none() && !self.anchor.can_reserve() {
            warnings.push(ConfigWarning {
                key: "exclusive_zone",
                message: "a positive zone counts only for a surface anchored to one edge, or \
                          to one edge and both edges across it; with this `anchor` the \
                          compositor treats it as 0 (name an `exclusive_edge` for a corner)"
                    .into(),
            });
        }
        Ok(warnings)
    }
}

/// The edge names the Python layer accepts for `anchor` and `exclusive_edge`.
pub fn parse_anchor<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<Anchors, ConfigError> {
    let mut bits = 0;
    for name in names {
        let Some(bit) = Anchors::edge(name) else {
            return Err(error(
                "anchor",
                format!(
                    "unknown edge {name:?}; valid: {}",
                    Anchors::NAMES.join(", ")
                ),
            ));
        };
        bits |= bit;
    }
    Ok(Anchors(bits))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> LayerConfig {
        LayerConfig {
            namespace: "tesserae-shell.bar".into(),
            anchor: Anchors(Anchors::TOP | Anchors::LEFT | Anchors::RIGHT),
            size: (0, 32),
            exclusive_zone: 32,
            ..Default::default()
        }
    }

    #[test]
    fn a_bar_is_valid_and_says_nothing() {
        assert_eq!(bar().validate(), Ok(vec![]));
    }

    #[test]
    fn a_stretched_axis_needs_both_opposite_edges() {
        let mut c = bar();
        c.anchor = Anchors(Anchors::TOP | Anchors::LEFT);
        let e = c.validate().unwrap_err();
        assert_eq!(e.key, "size");
        assert!(e.message.contains("left") && e.message.contains("right"));
        let mut c = bar();
        c.size = (200, 0);
        assert_eq!(c.validate().unwrap_err().key, "size");
        c.anchor = Anchors(Anchors::TOP | Anchors::BOTTOM | Anchors::LEFT);
        assert_eq!(c.validate(), Ok(vec![]));
        // Both axes stretched: a whole-output surface.
        let mut c = bar();
        c.size = (0, 0);
        c.anchor = Anchors(Anchors::TOP | Anchors::BOTTOM | Anchors::LEFT | Anchors::RIGHT);
        c.exclusive_zone = -1;
        assert_eq!(c.validate(), Ok(vec![]));
    }

    #[test]
    fn an_exclusive_edge_must_be_an_anchor() {
        let mut c = bar();
        c.exclusive_edge = Some(Anchors::BOTTOM);
        assert_eq!(c.validate().unwrap_err().key, "exclusive_edge");
        c.exclusive_edge = Some(Anchors::TOP);
        assert_eq!(c.validate(), Ok(vec![]));
    }

    #[test]
    fn a_zone_the_protocol_would_ignore_is_a_warning_not_an_error() {
        let mut c = bar();
        c.anchor = Anchors(Anchors::TOP | Anchors::LEFT);
        c.size = (200, 32);
        let w = c.validate().unwrap();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].key, "exclusive_zone");
        // Naming the edge makes it a corner clock that reserves the top.
        c.exclusive_edge = Some(Anchors::TOP);
        assert_eq!(c.validate(), Ok(vec![]));
        // No zone, no warning.
        c.exclusive_edge = None;
        c.exclusive_zone = 0;
        assert_eq!(c.validate(), Ok(vec![]));
    }

    #[test]
    fn which_anchor_sets_can_reserve_space() {
        let a = |bits| Anchors(bits).can_reserve();
        assert!(a(Anchors::TOP));
        assert!(a(Anchors::TOP | Anchors::LEFT | Anchors::RIGHT));
        assert!(a(Anchors::LEFT | Anchors::TOP | Anchors::BOTTOM));
        assert!(!a(0));
        assert!(!a(Anchors::TOP | Anchors::LEFT));
        assert!(!a(Anchors::TOP | Anchors::BOTTOM));
        assert!(!a(Anchors::TOP
            | Anchors::BOTTOM
            | Anchors::LEFT
            | Anchors::RIGHT));
    }

    #[test]
    fn the_namespace_and_the_zone_are_checked() {
        let mut c = bar();
        c.namespace = String::new();
        assert_eq!(c.validate().unwrap_err().key, "namespace");
        let mut c = bar();
        c.exclusive_zone = -2;
        assert_eq!(c.validate().unwrap_err().key, "exclusive_zone");
        c.exclusive_zone = -1;
        assert_eq!(c.validate(), Ok(vec![]));
    }

    #[test]
    fn names_parse_and_round_trip() {
        for name in LayerKind::NAMES {
            assert_eq!(LayerKind::parse(name).unwrap().name(), name);
        }
        for name in KeyboardMode::NAMES {
            assert_eq!(KeyboardMode::parse(name).unwrap().name(), name);
        }
        assert!(LayerKind::parse("above").is_none());
        assert!(KeyboardMode::parse("always").is_none());
        let a = parse_anchor(["top", "left", "right"]).unwrap();
        assert_eq!(a.names(), vec!["top", "left", "right"]);
        assert_eq!(parse_anchor(["middle"]).unwrap_err().key, "anchor");
        assert!(parse_anchor([]).unwrap().is_empty());
    }
}
