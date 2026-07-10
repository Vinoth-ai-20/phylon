//! Organism visual-instance builders (Phase 7, W2a) — the "what to draw"
//! half of `render.rs`'s per-node and per-spring loops, extracted so those
//! loops orchestrate (gather data, call a builder, push the result) rather
//! than compute biological-visual semantics inline.
//!
//! Every function here is pure: it takes already-looked-up data (a
//! component, a resolved position, a resolved scalar) and returns the
//! instance(s) the caller should push — never a `World` query, never a
//! `PhylonApp` field access. This is a verbatim extraction of `render.rs`'s
//! prior inline logic (Phase 7 W2 is architectural separation, not a visual
//! or behavioral change) — every threshold, color, and gating condition
//! moves unchanged.

/// Pellet-like entity radii (Phase 7 W2b's `pellet_like_instances` call
/// sites) — named and shared (Phase 8, Epic 8.4) so ray-vs-capsule picking
/// hit-tests against exactly the same radius the renderer draws, rather
/// than a second, independently-tuned literal.
pub(crate) const FOOD_PELLET_RADIUS: f32 = 2.5;
/// See [`FOOD_PELLET_RADIUS`].
pub(crate) const MINERAL_PELLET_RADIUS: f32 = 2.0;
/// See [`FOOD_PELLET_RADIUS`].
pub(crate) const CORPSE_RADIUS: f32 = 4.0;

/// Low-health ring — primary tier, always visible (not gated behind
/// `debug_structural`). `None` above 40% health, matching the prior
/// inline behavior where nothing is drawn for the common healthy case.
pub(crate) fn health_ring_instance(
    health: &metabolism::Health,
    pos: [f32; 3],
    node_radius: f32,
) -> Option<rendering::DebugInstance> {
    let fraction = if health.max > 0.0 {
        (health.current / health.max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if fraction >= 0.40 {
        return None;
    }
    let token = if fraction < 0.15 {
        ui::theme::BAD
    } else {
        ui::theme::WARN
    };
    let [r, g, b, _] = token.to_normalized_gamma_f32();
    Some(rendering::DebugInstance {
        pos_a: pos,
        pos_b: pos,
        color: [r, g, b, 0.6],
        radius: 10.0 * (node_radius / 5.0),
        segment_type: 99,
    })
}

/// Disease badge — primary tier, always visible, offset up-and-left from
/// the head position so it never blends with the health ring into an
/// ambiguous combined color. Always produces an instance (the caller only
/// invokes this when an `Infection` component is present).
pub(crate) fn disease_badge_instance(
    infection: &ecology::disease::Infection,
    avg_severity: f32,
    health_fraction: f32,
    pos: [f32; 3],
    node_radius: f32,
) -> rendering::DebugInstance {
    let is_critical = avg_severity > 0.70 || health_fraction < 0.15;

    let (color, alpha, radius) = match infection.state {
        ecology::disease::InfectionState::Incubating => ([0.6, 0.6, 0.65], 0.25, 4.0),
        ecology::disease::InfectionState::Infectious if is_critical => {
            let [r, g, b, _] = ui::theme::BAD.to_normalized_gamma_f32();
            ([r, g, b], 0.85, 9.0)
        }
        ecology::disease::InfectionState::Infectious => {
            let purple = ecology::Diet::Decomposer.standard_color();
            (purple, 0.4 + avg_severity * 0.4, 5.0 + avg_severity * 3.0)
        }
        ecology::disease::InfectionState::Recovered => {
            let [r, g, b, _] = ui::theme::GOOD.to_normalized_gamma_f32();
            ([r, g, b], 0.5, 4.0)
        }
    };
    let offset = 12.0 * (node_radius / 5.0);
    let offset_pos = [pos[0] - offset, pos[1] - offset, pos[2]];
    rendering::DebugInstance {
        pos_a: offset_pos,
        pos_b: offset_pos,
        color: [color[0], color[1], color[2], alpha],
        radius,
        segment_type: 99,
    }
}

/// Structural segment-type debug dot — debug-tier only, gated externally by
/// `should_draw_debug`.
pub(crate) fn segment_debug_dot_instance(
    node: &physics::ParticleNode,
    node_radius: f32,
) -> rendering::DebugInstance {
    let pos: [f32; 3] = node.position.into();
    rendering::DebugInstance {
        pos_a: pos,
        pos_b: pos,
        color: match node.segment_type {
            0 => [1.000, 1.000, 1.000, 1.0], // Head - Absolute White #FFFFFF
            2 => [1.000, 0.033, 0.133, 1.0], // Muscle - Actuation Pink #FF3366
            3 => [1.000, 0.319, 0.000, 1.0], // Tail - Terminal Orange #FF9900
            4 => [0.000, 0.784, 1.000, 1.0], // Fin - Passive Cyan #00E5FF
            5 => [1.000, 0.000, 0.400, 1.0], // Vascular - Circulatory Magenta #FF0066
            6 => [0.600, 0.200, 1.000, 1.0], // Ganglion - Neural Violet #9933FF
            7 => [1.000, 0.843, 0.000, 1.0], // Germinal - Germ-line Gold #FFD700
            _ => [0.000, 0.784, 1.000, 1.0], // Torso - Passive Cyan #00E5FF
        },
        radius: if node.segment_type == 4 {
            3.0 * (node_radius / 5.0)
        } else {
            node_radius
        },
        segment_type: node.segment_type,
    }
}

/// Ecological-category ring around the head — debug-tier only. `None` for
/// categories with no distinct ring color (`EcologicalCategory::None` and
/// any future variant this match doesn't yet cover).
pub(crate) fn category_ring_instance(
    category: &ecology::EcologicalCategory,
    node: &physics::ParticleNode,
    node_radius: f32,
) -> Option<rendering::DebugInstance> {
    let ring_color = match category {
        ecology::EcologicalCategory::Keystone => Some([1.0, 0.84, 0.0, 1.0]), // Gold
        ecology::EcologicalCategory::Indicator => Some([0.0, 1.0, 1.0, 1.0]), // Cyan
        ecology::EcologicalCategory::Endemic => Some([0.0, 0.5, 0.5, 1.0]),   // Teal
        ecology::EcologicalCategory::Invasive => Some([1.0, 0.0, 1.0, 1.0]),  // Magenta
        _ => None,
    };
    let col = ring_color?;
    let pos: [f32; 3] = node.position.into();
    Some(rendering::DebugInstance {
        pos_a: pos,
        pos_b: pos,
        color: [col[0], col[1], col[2], 0.3],
        radius: 12.0 * (node_radius / 5.0),
        segment_type: 99,
    })
}

/// Colony/migration link — a spring whose two endpoints belong to different
/// organisms. Population-wide, always visible (caller only invokes this
/// when `org_a != org_b`).
pub(crate) fn colony_link_instance(
    pos_a: [f32; 3],
    pos_b: [f32; 3],
    skin_thickness: f32,
) -> rendering::DebugInstance {
    let [r, g, b, _] = ui::theme::ACCENT.to_normalized_gamma_f32();
    rendering::DebugInstance {
        pos_a,
        pos_b,
        color: [r, g, b, 0.55],
        radius: 5.0 * (skin_thickness / 3.0),
        segment_type: 99,
    }
}

/// The highlight radius for a hovered/selected bone — depends on the same
/// fin/constraint-type rules the main bone-visual tiers below use.
fn highlight_radius(
    is_fin: bool,
    constraint_type: physics::ConstraintType,
    skin_thickness: f32,
) -> f32 {
    if is_fin || constraint_type == physics::ConstraintType::Passive {
        4.0 * (skin_thickness / 3.0)
    } else if constraint_type == physics::ConstraintType::Elastic {
        6.0 * (skin_thickness / 3.0)
    } else {
        8.0 * (skin_thickness / 3.0)
    }
}

/// Hover/selection bone highlight instances. Returns `(hover, selected)` —
/// either may be `None` if the bone isn't in that highlight set.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bone_highlight_instances(
    pos_a: [f32; 3],
    pos_b: [f32; 3],
    is_fin: bool,
    constraint_type: physics::ConstraintType,
    skin_thickness: f32,
    is_hovered: bool,
    is_selected: bool,
) -> (
    Option<rendering::CapsuleInstance>,
    Option<rendering::CapsuleInstance>,
) {
    let radius = highlight_radius(is_fin, constraint_type, skin_thickness);
    let hover = is_hovered.then_some(rendering::CapsuleInstance {
        pos_a,
        pos_b,
        radius,
        color: [0.0, 1.0, 0.0],
        health: 1.0,
    });
    let selected = is_selected.then_some(rendering::CapsuleInstance {
        pos_a,
        pos_b,
        radius,
        color: [1.0, 1.0, 1.0],
        health: 1.0,
    });
    (hover, selected)
}

/// Which structural tier a spring's "main" skin/bone visual belongs to —
/// the three near-duplicate branches `render.rs` used to fork on inline
/// (passive tail / elastic muscle / rigid-or-rotational), now one enum
/// driving one builder function instead of three copies of the same
/// SDF-instance/debug-instance construction.
pub(crate) enum BoneKind {
    /// Thin, dimmed — a passive, non-fin ("tail") bone.
    PassiveTail,
    /// Medium weight, undimmed — an elastic ("muscle") bone.
    ElasticMuscle,
    /// Rigid or rotational bone; thinner if it's a fin.
    RigidOrRotational { is_fin: bool },
}

/// The main skin/bone visual for one spring — both the "real" SDF instance
/// (gated by `should_draw_sdf && bone_visible`) and the debug-tier instance
/// (gated by `should_draw_debug`). Either or both may be `None` if their
/// gate doesn't pass.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bone_visual_instances(
    kind: BoneKind,
    pos_a3: [f32; 3],
    pos_b3: [f32; 3],
    opt_color: Option<[f32; 3]>,
    health_fraction: f32,
    spotlight_factor: f32,
    growth_scale: f32,
    skin_thickness: f32,
    bone_line_thickness: f32,
    should_draw_sdf: bool,
    should_draw_debug: bool,
    bone_visible: bool,
) -> (
    Option<rendering::CapsuleInstance>,
    Option<rendering::DebugInstance>,
) {
    let (radius, color) = match kind {
        BoneKind::PassiveTail => {
            let color = opt_color
                .map(|c| [c[0] * 0.6, c[1] * 0.6, c[2] * 0.6])
                .unwrap_or([0.4, 0.4, 0.4]);
            (4.0 * (skin_thickness / 3.0) * growth_scale, color)
        }
        BoneKind::ElasticMuscle => {
            let color = opt_color.unwrap_or([0.5, 0.5, 0.8]);
            (6.0 * (skin_thickness / 3.0) * growth_scale, color)
        }
        BoneKind::RigidOrRotational { is_fin } => {
            let color = opt_color.unwrap_or([0.8, 0.8, 0.8]);
            let base_radius = if is_fin { 4.0 } else { 8.0 };
            (base_radius * (skin_thickness / 3.0) * growth_scale, color)
        }
    };
    let health = health_fraction * spotlight_factor;

    let sdf = (should_draw_sdf && bone_visible).then_some(rendering::CapsuleInstance {
        pos_a: pos_a3,
        pos_b: pos_b3,
        radius,
        color,
        health,
    });
    let debug = should_draw_debug.then_some(rendering::DebugInstance {
        pos_a: pos_a3,
        pos_b: pos_b3,
        color: [0.246, 0.287, 0.434, 0.4],
        radius: bone_line_thickness,
        segment_type: 99,
    });
    (sdf, debug)
}

/// The four instances a point-entity ("pellet-like": food, mineral, or
/// corpse) may produce — debug dot, "real" SDF dot, hover highlight, and
/// selection highlight — every field of which was, before W2b, duplicated
/// verbatim three times (once per entity kind) with only `debug_color`/
/// `sdf_color`/`radius` differing. `radius` is shared across all four
/// instances, matching the prior per-kind code (each kind used one literal
/// for its debug/sdf/hover/selected radius alike).
pub(crate) struct PelletInstances {
    pub(crate) debug: Option<rendering::DebugInstance>,
    pub(crate) sdf: Option<rendering::CapsuleInstance>,
    pub(crate) hover: Option<rendering::CapsuleInstance>,
    pub(crate) selected: Option<rendering::CapsuleInstance>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn pellet_like_instances(
    pos3: [f32; 3],
    debug_color: [f32; 4],
    sdf_color: [f32; 3],
    radius: f32,
    should_draw_debug: bool,
    should_draw_sdf: bool,
    bone_visible: bool,
    is_hovered: bool,
    is_selected: bool,
) -> PelletInstances {
    let debug = should_draw_debug.then_some(rendering::DebugInstance {
        pos_a: pos3,
        pos_b: pos3,
        color: debug_color,
        radius,
        segment_type: 0,
    });
    let sdf = (should_draw_sdf && bone_visible).then_some(rendering::CapsuleInstance {
        pos_a: pos3,
        pos_b: pos3,
        radius,
        color: sdf_color,
        health: 1.0,
    });
    let hover = is_hovered.then_some(rendering::CapsuleInstance {
        pos_a: pos3,
        pos_b: pos3,
        radius,
        color: [0.0, 1.0, 0.0],
        health: 1.0,
    });
    let selected = is_selected.then_some(rendering::CapsuleInstance {
        pos_a: pos3,
        pos_b: pos3,
        radius,
        color: [1.0, 1.0, 1.0],
        health: 1.0,
    });
    PelletInstances {
        debug,
        sdf,
        hover,
        selected,
    }
}
