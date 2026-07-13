use crate::game::{GameData, Lander, SimulationSettings};
use crate::terrain::TerrainData;
use crate::GameState;
use bevy::prelude::*;

const MAX_MAIN_THRUST: f32 = 150.0;
const MAX_ANGULAR_THRUST: f32 = 3.0;
const FUEL_CONSUMPTION_RATE: f32 = 10.0;
const PREVIEW_DT: f32 = 1.0 / 30.0;

fn throttle_response(current: f32, target: f32, dt: f32) -> f32 {
    let response_time = if target.abs() > current.abs() {
        0.28
    } else {
        0.18
    };
    current + (target - current) * (1.0 - (-dt / response_time).exp())
}

#[derive(Clone, Copy, Debug)]
pub struct ManeuverNode {
    pub id: u32,
    /// Seconds after execution is armed.
    pub at: f32,
    pub duration: f32,
    /// Signed component along the instantaneous velocity direction.
    pub prograde: f32,
    /// Signed component away from the nearest gravitational body.
    pub radial: f32,
    /// Additional signed attitude input, in the range -1..1.
    pub rotation: f32,
    /// Engine command in the range 0..1.
    pub throttle: f32,
}

impl ManeuverNode {
    fn sanitize(&mut self) {
        self.at = self.at.clamp(0.0, 86_400.0);
        self.duration = self.duration.clamp(0.1, 3_600.0);
        self.prograde = self.prograde.clamp(-1.0, 1.0);
        self.radial = self.radial.clamp(-1.0, 1.0);
        self.rotation = self.rotation.clamp(-1.0, 1.0);
        self.throttle = self.throttle.clamp(0.0, 1.0);
    }

    fn active_at(&self, elapsed: f32) -> bool {
        elapsed >= self.at && elapsed < self.at + self.duration
    }
}

#[derive(Resource, Debug)]
pub struct ManeuverPlan {
    pub enabled: bool,
    /// Execution is deliberately separate from editing so adding a node can
    /// never fire a thruster by accident.
    pub armed: bool,
    pub elapsed: f32,
    pub nodes: Vec<ManeuverNode>,
    pub selected: Option<u32>,
    next_id: u32,
}

impl Default for ManeuverPlan {
    fn default() -> Self {
        Self {
            enabled: false,
            armed: false,
            elapsed: 0.0,
            nodes: Vec::new(),
            selected: None,
            next_id: 1,
        }
    }
}

impl ManeuverPlan {
    pub fn add_node(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let at = self
            .nodes
            .last()
            .map_or(10.0, |node| node.at + node.duration + 5.0);
        self.nodes.push(ManeuverNode {
            id,
            at,
            duration: 2.0,
            prograde: 1.0,
            radial: 0.0,
            rotation: 0.0,
            throttle: 0.5,
        });
        self.selected = Some(id);
        id
    }

    pub fn selected(&self) -> Option<ManeuverNode> {
        let id = self.selected?;
        self.nodes.iter().find(|node| node.id == id).copied()
    }

    pub fn edit_selected(&mut self, mut replacement: ManeuverNode) {
        replacement.sanitize();
        if let Some(node) = self.nodes.iter_mut().find(|node| node.id == replacement.id) {
            *node = replacement;
            self.nodes.sort_by(|a, b| a.at.total_cmp(&b.at));
        }
    }

    pub fn select_relative(&mut self, delta: i8) {
        if self.nodes.is_empty() {
            self.selected = None;
            return;
        }
        let current = self
            .selected
            .and_then(|id| self.nodes.iter().position(|node| node.id == id))
            .unwrap_or(0) as isize;
        let next = (current + delta as isize).rem_euclid(self.nodes.len() as isize) as usize;
        self.selected = Some(self.nodes[next].id);
    }

    pub fn delete_selected(&mut self) {
        let Some(id) = self.selected else { return };
        let Some(index) = self.nodes.iter().position(|node| node.id == id) else {
            self.selected = None;
            return;
        };
        self.nodes.remove(index);
        self.selected = if self.nodes.is_empty() {
            self.armed = false;
            None
        } else {
            Some(self.nodes[index.min(self.nodes.len() - 1)].id)
        };
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.selected = None;
        self.armed = false;
        self.elapsed = 0.0;
    }

    pub fn set_armed(&mut self, armed: bool) {
        // Requiring a non-empty plan prevents a misleading armed state.
        self.armed = armed && !self.nodes.is_empty();
        self.elapsed = 0.0;
    }
}

pub struct ManeuverPlugin;

impl Plugin for ManeuverPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ManeuverPlan>().add_systems(
            Update,
            (maneuver_keyboard, draw_maneuver_preview).run_if(in_state(GameState::Playing)),
        );
    }
}

fn maneuver_keyboard(keys: Res<ButtonInput<KeyCode>>, mut plan: ResMut<ManeuverPlan>) {
    if keys.just_pressed(KeyCode::KeyN) {
        plan.enabled = !plan.enabled;
        if !plan.enabled {
            plan.armed = false;
        }
    }
    if !plan.enabled {
        return;
    }
    if keys.just_pressed(KeyCode::KeyK) {
        plan.add_node();
    }
    if keys.just_pressed(KeyCode::Enter) {
        let arm = !plan.armed;
        plan.set_armed(arm);
    }
    if keys.just_pressed(KeyCode::Backspace) {
        plan.clear();
    }
}

/// Runs in the main flight chain after manual input and before physics.
/// An armed node owns the controls only for its bounded execution window;
/// aborting immediately returns authority to the pilot.
pub(crate) fn apply_maneuver_inputs(
    time: Res<Time>,
    settings: Res<SimulationSettings>,
    terrain: Res<TerrainData>,
    mut plan: ResMut<ManeuverPlan>,
    mut game_data: ResMut<GameData>,
    mut lander: Query<(&Transform, &mut Lander)>,
) {
    if !plan.enabled || !plan.armed {
        return;
    }
    let dt = time.delta_secs() * settings.time_multiplier;
    plan.elapsed += dt;
    let Some(node) = plan
        .nodes
        .iter()
        .find(|node| node.active_at(plan.elapsed))
        .copied()
    else {
        if plan
            .nodes
            .iter()
            .all(|node| plan.elapsed >= node.at + node.duration)
        {
            plan.armed = false;
        }
        return;
    };
    if game_data.fuel <= 0.0 && !settings.infinite_fuel {
        plan.armed = false;
        return;
    }
    let Ok((transform, mut ship)) = lander.single_mut() else {
        return;
    };
    let position = transform.translation.truncate();
    let prograde = ship.velocity.try_normalize().unwrap_or(Vec2::Y);
    let nearest = terrain.planets.iter().min_by(|a, b| {
        position
            .distance_squared(a.center)
            .total_cmp(&position.distance_squared(b.center))
    });
    let radial = nearest
        .map(|body| (position - body.center).try_normalize().unwrap_or(Vec2::X))
        .unwrap_or(Vec2::X);
    let desired = (prograde * node.prograde + radial * node.radial)
        .try_normalize()
        .unwrap_or(prograde);
    let forward = (transform.rotation * Vec3::Y).truncate();
    let heading_error = forward.perp_dot(desired);
    // Manual-input processing already billed the residual throttle present at
    // the start of this frame. Only bill the planner's additional command.
    let already_billed =
        ship.main_thrust / MAX_MAIN_THRUST + 0.5 * ship.angular_thrust.abs() / MAX_ANGULAR_THRUST;
    let angular_target =
        (heading_error * 2.5 + node.rotation).clamp(-1.0, 1.0) * MAX_ANGULAR_THRUST;
    ship.angular_thrust = throttle_response(ship.angular_thrust, angular_target, dt);
    // Only burn once reasonably aligned. This guard is the execution-mode
    // equivalent of KSP's maneuver-node alignment indicator.
    let main_target = if forward.dot(desired) > 0.94 {
        node.throttle * MAX_MAIN_THRUST
    } else {
        0.0
    };
    ship.main_thrust = throttle_response(ship.main_thrust, main_target, dt);
    ship.reverse_thrust = 0.0;
    ship.left_thrust = 0.0;
    ship.right_thrust = 0.0;
    if !settings.infinite_fuel {
        let burn_fraction = ship.main_thrust / MAX_MAIN_THRUST
            + 0.5 * ship.angular_thrust.abs() / MAX_ANGULAR_THRUST;
        let additional_burn = (burn_fraction - already_billed).max(0.0);
        game_data.fuel = (game_data.fuel - FUEL_CONSUMPTION_RATE * additional_burn * dt).max(0.0);
    }
}

fn draw_maneuver_preview(
    plan: Res<ManeuverPlan>,
    settings: Res<SimulationSettings>,
    terrain: Res<TerrainData>,
    lander: Query<(&Transform, &Lander)>,
    mut gizmos: Gizmos,
) {
    if !plan.enabled || plan.nodes.is_empty() {
        return;
    }
    let Ok((transform, ship)) = lander.single() else {
        return;
    };
    let mut position = transform.translation.truncate();
    let mut velocity = ship.velocity;
    let timeline_start = if plan.armed { plan.elapsed } else { 0.0 };
    let timeline_end = plan
        .nodes
        .iter()
        .map(|node| node.at + node.duration)
        .fold(10.0_f32, f32::max)
        + 20.0;
    let horizon = (timeline_end - timeline_start).max(20.0);
    let steps = (horizon / PREVIEW_DT).ceil().min(12_000.0) as usize;
    let mut previous = position;
    let mut node_markers = Vec::new();
    let mut next_marker = plan.nodes.iter().position(|node| node.at >= timeline_start);
    for step in 1..=steps {
        let elapsed = step as f32 * PREVIEW_DT;
        let timeline_time = timeline_start + elapsed;
        let gravity = terrain.relativistic_gravity_at_time(position, velocity, elapsed)
            * settings.gravity_multiplier;
        let mut thrust = Vec2::ZERO;
        if let Some(node) = plan.nodes.iter().find(|node| node.active_at(timeline_time)) {
            let prograde = velocity.try_normalize().unwrap_or(Vec2::Y);
            let nearest = terrain.planets.iter().enumerate().min_by(|(_, a), (_, b)| {
                position
                    .distance_squared(a.center)
                    .total_cmp(&position.distance_squared(b.center))
            });
            let radial = nearest
                .map(|(index, _)| {
                    let center = terrain.body_center_at_time(index, elapsed);
                    (position - center).try_normalize().unwrap_or(Vec2::X)
                })
                .unwrap_or(Vec2::X);
            let direction = (prograde * node.prograde + radial * node.radial)
                .try_normalize()
                .unwrap_or(prograde);
            thrust = direction * node.throttle * MAX_MAIN_THRUST * settings.thrust_multiplier;
        }
        velocity += (gravity + thrust) / ship.mass * PREVIEW_DT;
        position += velocity * PREVIEW_DT;
        while let Some(index) = next_marker {
            if plan.nodes[index].at > timeline_time {
                break;
            }
            node_markers.push(position);
            next_marker = (index + 1 < plan.nodes.len()).then_some(index + 1);
        }
        if step % 3 == 0 {
            gizmos.line_2d(previous, position, Color::srgba(1.0, 0.3, 0.85, 0.8));
            previous = position;
        }
    }
    for marker in node_markers {
        gizmos.circle_2d(marker, 8.0, Color::srgb(1.0, 0.75, 0.2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arming_is_explicit_and_edits_are_clamped() {
        let mut plan = ManeuverPlan::default();
        plan.set_armed(true);
        assert!(!plan.armed);
        plan.add_node();
        let mut node = plan.selected().unwrap();
        node.throttle = 4.0;
        node.duration = -1.0;
        plan.edit_selected(node);
        assert_eq!(plan.selected().unwrap().throttle, 1.0);
        assert_eq!(plan.selected().unwrap().duration, 0.1);
        plan.set_armed(true);
        assert!(plan.armed);
    }

    #[test]
    fn node_selection_wraps_and_delete_keeps_a_valid_selection() {
        let mut plan = ManeuverPlan::default();
        let first = plan.add_node();
        let second = plan.add_node();
        assert_eq!(plan.selected, Some(second));
        plan.select_relative(1);
        assert_eq!(plan.selected, Some(first));
        plan.select_relative(-1);
        assert_eq!(plan.selected, Some(second));
        plan.delete_selected();
        assert_eq!(plan.nodes.len(), 1);
        assert_eq!(plan.selected, Some(first));
    }
}
