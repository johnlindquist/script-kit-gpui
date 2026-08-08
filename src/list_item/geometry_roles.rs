//! GEO-001 — Canonical geometry-role vocabulary for cross-layer measurement joins.
//!
//! A [`GeometryRole`] names WHAT a measured rectangle is, independent of its
//! numeric value. Two measurements are numerically comparable only when their
//! roles are identical ([`GeometryRole::comparable_to`]). Related roles carry
//! containment/exclusion/sibling contracts through
//! [`GEOMETRY_ROLE_RELATIONS`], but a relation NEVER makes two roles
//! numerically interchangeable: `FooterNativeHost` containing `FooterRail`
//! does not permit joining their heights, and unequal dimensions between
//! related roles are not drift.
//!
//! This is the lane-owned Rust authority for the role vocabulary. The
//! protocol layer (`crate::protocol::GeometryRole`) carries the serialized
//! camelCase names; [`GeometryRole::as_str`] uses the identical strings so a
//! role survives Rust → protocol → probe joins byte-for-byte. The
//! [`GeometryRole::RenderedFooterReservation`] variant is a derived
//! safe-viewport exclusion that is NOT an alias of any painted footer owner;
//! its protocol projection is pending (integration request IR-01) and until it
//! lands [`GeometryRole::to_protocol`] maps it to `Other` rather than lying
//! with a painted footer role.
//!
//! GEO-001 changes no visual value: this module only names, relates, and tags.

/// Semantic role of a measured rectangle.
///
/// Roles decide comparability. Never infer a role from an arbitrary display
/// name; producers must tag measurements at the source.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeometryRole {
    WindowBackdrop,
    MainHeaderChrome,
    ContextZone,
    InputControl,
    ContentViewport,
    RowSlot,
    SectionSlot,
    FooterNativeHost,
    FooterRail,
    FooterActionRow,
    FooterActionSlot,
    KeycapInnerFrame,
    /// Derived safe-viewport exclusion. This is not an alias of any footer
    /// owner (`FooterNativeHost`, `FooterRail`, `FooterActionRow`): it is the
    /// amount of viewport the content layer must keep clear of the rendered
    /// footer, carried as its own measurement.
    RenderedFooterReservation,
    PopupShell,
    PopupAnchor,
    TextLineBox,
    GlyphBounds,
    FocusRing,
}

impl GeometryRole {
    /// Stable serialized name; identical to the protocol camelCase strings.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WindowBackdrop => "windowBackdrop",
            Self::MainHeaderChrome => "mainHeaderChrome",
            Self::ContextZone => "contextZone",
            Self::InputControl => "inputControl",
            Self::ContentViewport => "contentViewport",
            Self::RowSlot => "rowSlot",
            Self::SectionSlot => "sectionSlot",
            Self::FooterNativeHost => "footerNativeHost",
            Self::FooterRail => "footerRail",
            Self::FooterActionRow => "footerActionRow",
            Self::FooterActionSlot => "footerActionSlot",
            Self::KeycapInnerFrame => "keycapInnerFrame",
            Self::RenderedFooterReservation => "renderedFooterReservation",
            Self::PopupShell => "popupShell",
            Self::PopupAnchor => "popupAnchor",
            Self::TextLineBox => "textLineBox",
            Self::GlyphBounds => "glyphBounds",
            Self::FocusRing => "focusRing",
        }
    }

    /// Numeric comparability: identical roles only. Relations never widen this.
    pub const fn comparable_to(self, other: Self) -> bool {
        self as u32 == other as u32
    }

    /// Projection into the protocol enum where a variant exists.
    ///
    /// `RenderedFooterReservation` has no protocol variant yet (IR-01);
    /// mapping it to any painted footer role would be a wrong-owner join, so
    /// it degrades to `Other` until the protocol variant lands.
    pub fn to_protocol(self) -> crate::protocol::GeometryRole {
        use crate::protocol::GeometryRole as P;
        match self {
            Self::WindowBackdrop => P::WindowBackdrop,
            Self::MainHeaderChrome => P::MainHeaderChrome,
            Self::ContextZone => P::ContextZone,
            Self::InputControl => P::InputControl,
            Self::ContentViewport => P::ContentViewport,
            Self::RowSlot => P::RowSlot,
            Self::SectionSlot => P::SectionSlot,
            Self::FooterNativeHost => P::FooterNativeHost,
            Self::FooterRail => P::FooterRail,
            Self::FooterActionRow => P::FooterActionRow,
            Self::FooterActionSlot => P::FooterActionSlot,
            Self::KeycapInnerFrame => P::KeycapInnerFrame,
            // Pending protocol variant (IR-01). Never a painted footer role.
            Self::RenderedFooterReservation => P::Other,
            Self::PopupShell => P::PopupShell,
            Self::PopupAnchor => P::PopupAnchor,
            Self::TextLineBox => P::TextLineBox,
            Self::GlyphBounds => P::GlyphBounds,
            Self::FocusRing => P::FocusRing,
        }
    }
}

/// Kind of structural relationship between two roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryRelationKind {
    /// Left encloses right. Enclosure does not make dimensions comparable.
    Contains,
    /// Left's usable area excludes right's reservation.
    Excludes,
    /// Peers in one layout axis; never aliases of each other.
    Sibling,
}

/// One declared relationship in the canonical relation table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeometryRoleRelation {
    pub left: GeometryRole,
    pub relation: GeometryRelationKind,
    pub right: GeometryRole,
}

/// Canonical relationship table (program plan Step 9, GEO-001).
pub const GEOMETRY_ROLE_RELATIONS: &[GeometryRoleRelation] = &[
    GeometryRoleRelation {
        left: GeometryRole::FooterNativeHost,
        relation: GeometryRelationKind::Contains,
        right: GeometryRole::FooterRail,
    },
    GeometryRoleRelation {
        left: GeometryRole::FooterRail,
        relation: GeometryRelationKind::Contains,
        right: GeometryRole::FooterActionRow,
    },
    GeometryRoleRelation {
        left: GeometryRole::FooterActionRow,
        relation: GeometryRelationKind::Contains,
        right: GeometryRole::FooterActionSlot,
    },
    GeometryRoleRelation {
        left: GeometryRole::FooterActionSlot,
        relation: GeometryRelationKind::Contains,
        right: GeometryRole::KeycapInnerFrame,
    },
    GeometryRoleRelation {
        left: GeometryRole::ContentViewport,
        relation: GeometryRelationKind::Excludes,
        right: GeometryRole::RenderedFooterReservation,
    },
    GeometryRoleRelation {
        left: GeometryRole::RowSlot,
        relation: GeometryRelationKind::Sibling,
        right: GeometryRole::SectionSlot,
    },
];

/// Look up the declared relation between two roles, if any.
pub fn declared_relation(left: GeometryRole, right: GeometryRole) -> Option<GeometryRelationKind> {
    GEOMETRY_ROLE_RELATIONS
        .iter()
        .find(|entry| entry.left == left && entry.right == right)
        .map(|entry| entry.relation)
}

/// Result of asking whether two role-tagged measurements may be numerically
/// compared. Related-but-different roles are `RoleMismatch`, never "drift".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeometryComparability {
    Comparable,
    RoleMismatch,
}

pub const fn geometry_comparability(
    left: GeometryRole,
    right: GeometryRole,
) -> GeometryComparability {
    if left.comparable_to(right) {
        GeometryComparability::Comparable
    } else {
        GeometryComparability::RoleMismatch
    }
}

/// A metric value tagged with its role, stable measurement ID, and source.
///
/// Tagging carries metadata only; it never changes the value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoleTaggedMetric {
    pub measurement_id: &'static str,
    pub role: GeometryRole,
    pub value_px: f32,
    pub source_path: &'static str,
    pub source_symbol: &'static str,
}

impl RoleTaggedMetric {
    /// Compare two same-role metrics. Different roles are a `RoleMismatch`
    /// regardless of values; same-role values must match exactly (sub-pixel
    /// deltas — including one pixel — are detected, never tolerated).
    pub fn compare(&self, other: &RoleTaggedMetric) -> RoleTaggedComparison {
        match geometry_comparability(self.role, other.role) {
            GeometryComparability::RoleMismatch => RoleTaggedComparison::RoleMismatch,
            GeometryComparability::Comparable => {
                if (self.value_px - other.value_px).abs() == 0.0 {
                    RoleTaggedComparison::Equal
                } else {
                    RoleTaggedComparison::ValueDelta {
                        delta_px: other.value_px - self.value_px,
                    }
                }
            }
        }
    }
}

/// Outcome of a role-tagged comparison.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RoleTaggedComparison {
    Equal,
    ValueDelta { delta_px: f32 },
    RoleMismatch,
}

/// Version string for the role vocabulary carried into probe receipts.
pub const GEOMETRY_ROLE_VOCABULARY_VERSION: &str = "geo-roles.v1";

/// All roles, for exhaustive export/enumeration without a wildcard match.
pub const ALL_GEOMETRY_ROLES: &[GeometryRole] = &[
    GeometryRole::WindowBackdrop,
    GeometryRole::MainHeaderChrome,
    GeometryRole::ContextZone,
    GeometryRole::InputControl,
    GeometryRole::ContentViewport,
    GeometryRole::RowSlot,
    GeometryRole::SectionSlot,
    GeometryRole::FooterNativeHost,
    GeometryRole::FooterRail,
    GeometryRole::FooterActionRow,
    GeometryRole::FooterActionSlot,
    GeometryRole::KeycapInnerFrame,
    GeometryRole::RenderedFooterReservation,
    GeometryRole::PopupShell,
    GeometryRole::PopupAnchor,
    GeometryRole::TextLineBox,
    GeometryRole::GlyphBounds,
    GeometryRole::FocusRing,
];

#[cfg(test)]
mod geometry_roles_contract_tests {
    use super::*;

    /// The footer ownership chain is containment, not equality: each level may
    /// (and does) have distinct numeric bounds while the relation holds.
    #[test]
    fn footer_hierarchy_allows_distinct_numeric_bounds() {
        let chain = [
            (GeometryRole::FooterNativeHost, 36.0_f32),
            (GeometryRole::FooterRail, 32.0),
            (GeometryRole::FooterActionRow, 26.0),
            (GeometryRole::FooterActionSlot, 22.0),
            (GeometryRole::KeycapInnerFrame, 16.0),
        ];
        for window in chain.windows(2) {
            let (outer_role, outer_h) = window[0];
            let (inner_role, inner_h) = window[1];
            assert_eq!(
                declared_relation(outer_role, inner_role),
                Some(GeometryRelationKind::Contains),
                "{} must contain {}",
                outer_role.as_str(),
                inner_role.as_str()
            );
            // Distinct numeric values are legitimate under Contains…
            assert_ne!(outer_h, inner_h);
            // …and containment never grants numeric comparability.
            assert_eq!(
                geometry_comparability(outer_role, inner_role),
                GeometryComparability::RoleMismatch
            );
        }
    }

    /// The content viewport excludes the rendered footer reservation; the
    /// reservation is its own derived role, not an alias of a painted owner.
    #[test]
    fn content_viewport_excludes_rendered_footer_reservation() {
        assert_eq!(
            declared_relation(
                GeometryRole::ContentViewport,
                GeometryRole::RenderedFooterReservation
            ),
            Some(GeometryRelationKind::Excludes)
        );
        for painted_owner in [
            GeometryRole::FooterNativeHost,
            GeometryRole::FooterRail,
            GeometryRole::FooterActionRow,
        ] {
            assert!(
                !GeometryRole::RenderedFooterReservation.comparable_to(painted_owner),
                "renderedFooterReservation must not be joined to {}",
                painted_owner.as_str()
            );
        }
    }

    /// RowSlot and SectionSlot are siblings — never aliases, never comparable.
    #[test]
    fn row_and_section_are_siblings_not_aliases() {
        assert_eq!(
            declared_relation(GeometryRole::RowSlot, GeometryRole::SectionSlot),
            Some(GeometryRelationKind::Sibling)
        );
        assert_eq!(
            geometry_comparability(GeometryRole::RowSlot, GeometryRole::SectionSlot),
            GeometryComparability::RoleMismatch
        );
        // Equal numeric values still do not alias the roles.
        let row = RoleTaggedMetric {
            measurement_id: "list:row-slot",
            role: GeometryRole::RowSlot,
            value_px: 32.0,
            source_path: "src/list_item/mod.rs",
            source_symbol: "effective_list_item_height_for_theme",
        };
        let section = RoleTaggedMetric {
            measurement_id: "list:section-slot",
            role: GeometryRole::SectionSlot,
            value_px: 32.0,
            source_path: "src/list_item/mod.rs",
            source_symbol: "effective_section_header_height_for_theme",
        };
        assert_eq!(row.compare(&section), RoleTaggedComparison::RoleMismatch);
    }

    /// Comparing a measurement against the wrong owner is a RoleMismatch,
    /// never a numeric verdict — even when the numbers happen to agree.
    #[test]
    fn wrong_owner_is_not_comparable() {
        let host = RoleTaggedMetric {
            measurement_id: "footer:native-host",
            role: GeometryRole::FooterNativeHost,
            value_px: 36.0,
            source_path: "src/window_resize/mod.rs",
            source_symbol: "NATIVE_MAIN_WINDOW_FOOTER_HEIGHT",
        };
        let action_row = RoleTaggedMetric {
            measurement_id: "footer:action-row",
            role: GeometryRole::FooterActionRow,
            value_px: 36.0,
            source_path: "src/components/footer_chrome.rs",
            source_symbol: "current_main_menu_footer_metrics",
        };
        assert_eq!(
            host.compare(&action_row),
            RoleTaggedComparison::RoleMismatch
        );
    }

    /// Same-role measurements with a one-pixel delta are detected as a value
    /// delta (an evaluable failure for exact joins), never silently equal.
    #[test]
    fn same_role_one_pixel_delta_is_detected() {
        let model = RoleTaggedMetric {
            measurement_id: "arg:row-slot",
            role: GeometryRole::RowSlot,
            value_px: 44.0,
            source_path: "src/window_resize/arg_layout.rs",
            source_symbol: "ResolvedArgLayout::row_slot_height",
        };
        let rendered_off_by_one = RoleTaggedMetric {
            value_px: 43.0,
            ..model
        };
        assert_eq!(
            model.compare(&rendered_off_by_one),
            RoleTaggedComparison::ValueDelta { delta_px: -1.0 }
        );
    }

    /// Tagging is metadata-only: the tagged value is byte-identical to the
    /// source value it wraps, for every role.
    #[test]
    fn role_tagging_does_not_change_metric_values() {
        for (index, role) in ALL_GEOMETRY_ROLES.iter().enumerate() {
            let source_value = 7.0 + index as f32 * 0.5;
            let tagged = RoleTaggedMetric {
                measurement_id: "tagging:identity",
                role: *role,
                value_px: source_value,
                source_path: "test",
                source_symbol: "test",
            };
            assert_eq!(tagged.value_px.to_bits(), source_value.to_bits());
        }
    }

    /// The serialized names must match the protocol projection's serde names
    /// for every variant that has a protocol counterpart.
    #[test]
    fn role_names_round_trip_through_protocol_serialization() {
        for role in ALL_GEOMETRY_ROLES {
            if matches!(role, GeometryRole::RenderedFooterReservation) {
                // Pending protocol variant (IR-01); projected as Other.
                assert_eq!(role.to_protocol(), crate::protocol::GeometryRole::Other);
                continue;
            }
            let serialized = serde_json::to_string(&role.to_protocol()).expect("serializes");
            assert_eq!(serialized, format!("\"{}\"", role.as_str()));
        }
    }
}
