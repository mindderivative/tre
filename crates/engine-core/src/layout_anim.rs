//! 0.5.6 (#161): animating a node's layout properties.
//!
//! A layout property is part of the node's taffy `Style`, not of its paint, so
//! an animation writes the property's current value into the style each frame
//! and the tree lays out again. Only properties that hold a length in pixels
//! animate; one that is `auto` or a percentage has no number to start from
//! (width and height start from the size layout gave the node).

use taffy::prelude::{Dimension, LengthPercentage, LengthPercentageAuto, Style};
use taffy::style::{ExpandedDimension, ExpandedLengthPercentage, ExpandedLengthPercentageAuto};

use crate::animation::Animated;

/// One layout property that can be animated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutProp {
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    X,
    Y,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    MarginTop,
    MarginRight,
    MarginBottom,
    MarginLeft,
    RowGap,
    ColumnGap,
    FlexBasis,
}

impl LayoutProp {
    /// The properties a Python property name stands for: one, or the sides
    /// of a shorthand (`padding`, `margin`, `gap`). Empty for any other name.
    pub fn from_name(name: &str) -> Vec<LayoutProp> {
        use LayoutProp::*;
        match name {
            "width" => vec![Width],
            "height" => vec![Height],
            "min_width" => vec![MinWidth],
            "min_height" => vec![MinHeight],
            "max_width" => vec![MaxWidth],
            "max_height" => vec![MaxHeight],
            "x" => vec![X],
            "y" => vec![Y],
            "padding_top" => vec![PaddingTop],
            "padding_right" => vec![PaddingRight],
            "padding_bottom" => vec![PaddingBottom],
            "padding_left" => vec![PaddingLeft],
            "padding" => vec![PaddingTop, PaddingRight, PaddingBottom, PaddingLeft],
            "margin_top" => vec![MarginTop],
            "margin_right" => vec![MarginRight],
            "margin_bottom" => vec![MarginBottom],
            "margin_left" => vec![MarginLeft],
            "margin" => vec![MarginTop, MarginRight, MarginBottom, MarginLeft],
            "row_gap" => vec![RowGap],
            "column_gap" => vec![ColumnGap],
            "gap" => vec![RowGap, ColumnGap],
            "flex_basis" => vec![FlexBasis],
            _ => Vec::new(),
        }
    }

    /// The Python property name of this one property.
    pub fn name(self) -> &'static str {
        use LayoutProp::*;
        match self {
            Width => "width",
            Height => "height",
            MinWidth => "min_width",
            MinHeight => "min_height",
            MaxWidth => "max_width",
            MaxHeight => "max_height",
            X => "x",
            Y => "y",
            PaddingTop => "padding_top",
            PaddingRight => "padding_right",
            PaddingBottom => "padding_bottom",
            PaddingLeft => "padding_left",
            MarginTop => "margin_top",
            MarginRight => "margin_right",
            MarginBottom => "margin_bottom",
            MarginLeft => "margin_left",
            RowGap => "row_gap",
            ColumnGap => "column_gap",
            FlexBasis => "flex_basis",
        }
    }

    /// Whether the property can't be negative.
    pub fn non_negative(self) -> bool {
        !matches!(
            self,
            LayoutProp::X
                | LayoutProp::Y
                | LayoutProp::MarginTop
                | LayoutProp::MarginRight
                | LayoutProp::MarginBottom
                | LayoutProp::MarginLeft
        )
    }

    /// The property's value in pixels, or `None` when it is `auto` or a
    /// percentage.
    pub fn read(self, style: &Style) -> Option<f64> {
        use LayoutProp::*;
        let dimension = |d: Dimension| match ExpandedDimension::from(d) {
            ExpandedDimension::Length(v) => Some(f64::from(v)),
            _ => None,
        };
        let auto = |v: LengthPercentageAuto| match ExpandedLengthPercentageAuto::from(v) {
            ExpandedLengthPercentageAuto::Length(v) => Some(f64::from(v)),
            _ => None,
        };
        let plain = |v: LengthPercentage| match ExpandedLengthPercentage::from(v) {
            ExpandedLengthPercentage::Length(v) => Some(f64::from(v)),
            _ => None,
        };
        match self {
            Width => dimension(style.size.width),
            Height => dimension(style.size.height),
            MinWidth => auto(style.min_size.width),
            MinHeight => auto(style.min_size.height),
            MaxWidth => auto(style.max_size.width),
            MaxHeight => auto(style.max_size.height),
            X => auto(style.inset.left),
            Y => auto(style.inset.top),
            PaddingTop => plain(style.padding.top),
            PaddingRight => plain(style.padding.right),
            PaddingBottom => plain(style.padding.bottom),
            PaddingLeft => plain(style.padding.left),
            MarginTop => auto(style.margin.top),
            MarginRight => auto(style.margin.right),
            MarginBottom => auto(style.margin.bottom),
            MarginLeft => auto(style.margin.left),
            RowGap => plain(style.gap.height),
            ColumnGap => plain(style.gap.width),
            FlexBasis => dimension(style.flex_basis),
        }
    }

    /// Writes `pixels` into the style as the property's value.
    pub fn write(self, style: &mut Style, pixels: f64) {
        use LayoutProp::*;
        let v = pixels as f32;
        match self {
            Width => style.size.width = Dimension::length(v),
            Height => style.size.height = Dimension::length(v),
            MinWidth => style.min_size.width = LengthPercentageAuto::length(v),
            MinHeight => style.min_size.height = LengthPercentageAuto::length(v),
            MaxWidth => style.max_size.width = LengthPercentageAuto::length(v),
            MaxHeight => style.max_size.height = LengthPercentageAuto::length(v),
            X => style.inset.left = LengthPercentageAuto::length(v),
            Y => style.inset.top = LengthPercentageAuto::length(v),
            PaddingTop => style.padding.top = LengthPercentage::length(v),
            PaddingRight => style.padding.right = LengthPercentage::length(v),
            PaddingBottom => style.padding.bottom = LengthPercentage::length(v),
            PaddingLeft => style.padding.left = LengthPercentage::length(v),
            MarginTop => style.margin.top = LengthPercentageAuto::length(v),
            MarginRight => style.margin.right = LengthPercentageAuto::length(v),
            MarginBottom => style.margin.bottom = LengthPercentageAuto::length(v),
            MarginLeft => style.margin.left = LengthPercentageAuto::length(v),
            RowGap => style.gap.height = LengthPercentage::length(v),
            ColumnGap => style.gap.width = LengthPercentage::length(v),
            FlexBasis => style.flex_basis = Dimension::length(v),
        }
    }
}

/// A running (or just finished) animation of one layout property.
pub struct LayoutAnim {
    pub prop: LayoutProp,
    pub value: Animated<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_length_reads_back_and_auto_does_not() {
        let mut style = Style::default();
        assert_eq!(LayoutProp::Width.read(&style), None, "auto");
        LayoutProp::Width.write(&mut style, 120.0);
        assert_eq!(LayoutProp::Width.read(&style), Some(120.0));
        LayoutProp::PaddingLeft.write(&mut style, 8.0);
        assert_eq!(LayoutProp::PaddingLeft.read(&style), Some(8.0));
        assert_eq!(LayoutProp::PaddingTop.read(&style), Some(0.0));
        style.size.height = Dimension::percent(0.5);
        assert_eq!(LayoutProp::Height.read(&style), None, "a percentage");
    }

    #[test]
    fn shorthands_stand_for_their_sides() {
        assert_eq!(LayoutProp::from_name("padding").len(), 4);
        assert_eq!(LayoutProp::from_name("margin").len(), 4);
        assert_eq!(LayoutProp::from_name("gap").len(), 2);
        assert_eq!(LayoutProp::from_name("x"), vec![LayoutProp::X]);
        assert!(LayoutProp::from_name("opacity").is_empty());
        assert!(LayoutProp::from_name("font_size").is_empty());
    }
}
