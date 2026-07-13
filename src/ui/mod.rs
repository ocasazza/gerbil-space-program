mod bridge;

use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use panel_kit::{use_workspace, LayoutBuilder, Mode, PanelKind, PanelWin, WinState};
use serde::{Deserialize, Serialize};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

pub use bridge::WebUiBridgePlugin;
use bridge::{command, snapshot, UiCommand, UiScreen, UiSnapshot};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum GamePanel {
    Telemetry,
    FlightControls,
    Minimap,
    ManeuverPlanner,
    ShipDesigner,
    GameCanvas,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QuickTier {
    Camera,
    Simulation,
    Navigation,
    Planner,
}

#[derive(Clone, Copy, PartialEq)]
struct QuickGesture {
    active: bool,
    origin: (f64, f64),
    pointer: (f64, f64),
    tier: Option<QuickTier>,
    tier_slot: Option<usize>,
    submenu: (f64, f64),
    action_slot: Option<usize>,
}

impl Default for QuickGesture {
    fn default() -> Self {
        Self {
            active: false,
            origin: (0.0, 0.0),
            pointer: (0.0, 0.0),
            tier: None,
            tier_slot: None,
            submenu: (0.0, 0.0),
            action_slot: None,
        }
    }
}

impl PanelKind for GamePanel {
    fn title(self) -> &'static str {
        match self {
            Self::Telemetry => "Flight Telemetry",
            Self::FlightControls => "Flight & Simulation",
            Self::Minimap => "System Map",
            Self::ManeuverPlanner => "Maneuver Planner",
            Self::ShipDesigner => "Ship Design",
            Self::GameCanvas => "Game View",
        }
    }
}

fn default_layout() -> Vec<PanelWin<GamePanel>> {
    let mut layout = LayoutBuilder::new();
    vec![
        layout
            .at(GamePanel::Telemetry, 18.0, 18.0, 330.0, 250.0)
            .with_tile(1, 2),
        layout
            .at(GamePanel::FlightControls, 18.0, 286.0, 390.0, 590.0)
            .with_tile(1, 5),
        layout
            .at(GamePanel::Minimap, 426.0, 18.0, 370.0, 300.0)
            .with_tile(1, 3),
        layout
            .at(GamePanel::ManeuverPlanner, 426.0, 336.0, 410.0, 480.0)
            .with_tile(1, 4),
        layout
            .at(GamePanel::ShipDesigner, 426.0, 500.0, 410.0, 360.0)
            .with_tile(1, 5),
        layout
            .at(GamePanel::GameCanvas, 714.0, 18.0, 560.0, 360.0)
            .with_tile(2, 4),
    ]
}

pub fn launch() {
    // Dioxus web mounts to `#main` by default, matching index.html.
    dioxus::launch(App);
}

#[allow(non_snake_case)]
fn App() -> Element {
    // Version the key when the shipped layout changes so stale maximized
    // development layouts cannot hide the Bevy canvas on first load.
    let workspace = use_workspace("gerbil-space-program-ui-v8", default_layout);
    let mut initial_mode = workspace.mode;
    use_hook(move || initial_mode.set(Mode::Tiling));
    let mut current = use_signal(UiSnapshot::default);
    let canvas_panelled = use_signal(|| true);
    let mut previous_screen = use_signal(|| UiScreen::Loading);
    let quick_gesture = use_signal(QuickGesture::default);

    // The flight menu is a marking gesture, not persistent chrome. Capture the
    // middle button at window level so it works over the Bevy canvas, panel
    // controls, and SVG map alike. The first radial sector locks a category;
    // continuing the same drag into the child wheel selects its action.
    use_effect(move || {
        let Some(window) = web_sys::window() else {
            return;
        };

        let mut down_state = quick_gesture;
        let on_down =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                if event.button() != 1 {
                    return;
                }
                event.prevent_default();
                event.stop_propagation();
                down_state.set(QuickGesture {
                    active: true,
                    origin: (event.client_x() as f64, event.client_y() as f64),
                    pointer: (event.client_x() as f64, event.client_y() as f64),
                    ..QuickGesture::default()
                });
            });
        let _ = window.add_event_listener_with_callback_and_bool(
            "mousedown",
            on_down.as_ref().unchecked_ref(),
            true,
        );
        on_down.forget();

        let mut move_state = quick_gesture;
        let on_move =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                let mut gesture = move_state();
                if !gesture.active {
                    return;
                }
                event.prevent_default();
                gesture.pointer = (event.client_x() as f64, event.client_y() as f64);
                update_quick_gesture(&mut gesture);
                move_state.set(gesture);
            });
        let _ = window.add_event_listener_with_callback_and_bool(
            "mousemove",
            on_move.as_ref().unchecked_ref(),
            true,
        );
        on_move.forget();

        let mut up_state = quick_gesture;
        let latest = current;
        let mut canvas_mode = canvas_panelled;
        let on_up =
            Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |event: web_sys::MouseEvent| {
                if event.button() != 1 || !up_state().active {
                    return;
                }
                event.prevent_default();
                event.stop_propagation();
                let gesture = up_state();
                if let (Some(tier), Some(action)) = (gesture.tier, gesture.action_slot) {
                    execute_quick_action(tier, action, *latest.peek(), &mut canvas_mode);
                }
                up_state.set(QuickGesture::default());
            });
        let _ = window.add_event_listener_with_callback_and_bool(
            "mouseup",
            on_up.as_ref().unchecked_ref(),
            true,
        );
        on_up.forget();
    });

    use_future(move || async move {
        let mut last_canvas_size = None;
        loop {
            current.set(snapshot());
            if canvas_panelled() {
                let size = canvas_panel_size();
                if size != last_canvas_size && size.is_some() {
                    dispatch_canvas_resize();
                    last_canvas_size = size;
                }
            } else {
                last_canvas_size = None;
            }
            TimeoutFuture::new(50).await;
        }
    });

    let data = current();
    use_effect(move || {
        let screen = current().screen;
        let previous = *previous_screen.peek();
        if screen == UiScreen::Playing && previous != UiScreen::Playing {
            // A maximized non-game panel removes the Game View host from the
            // rendered workspace. Starting a mission must make that host
            // visible again before Bevy attempts to draw into its canvas.
            let mut panels = workspace.panels;
            for panel in panels.write().iter_mut() {
                if panel.state == WinState::Maximized
                    || (panel.kind == GamePanel::GameCanvas && panel.state == WinState::Minimized)
                {
                    panel.state = WinState::Floating;
                }
            }
            let mut mode = workspace.mode;
            mode.set(Mode::Tiling);
            relocate_game_canvas(canvas_panelled());
            dispatch_canvas_resize();
        }
        if screen != previous {
            previous_screen.set(screen);
        }
    });
    use_effect(move || relocate_game_canvas(canvas_panelled()));
    rsx! {
        style { {panel_kit::CSS} }
        style { {GAME_CSS} }
        div {
            class: workspace.root_class(),
            onmousemove: move |event| workspace.handle_mouse_move(&event),
            onmouseup: move |_| workspace.handle_mouse_up(),
            {workspace.render(move |kind, _| panel_body(kind, data, canvas_panelled))}
            {workspace.dock()}
        }
        if data.screen == UiScreen::Playing {
            QuickRadialMenu { gesture: quick_gesture(), data }
            if data.trajectory {
                div { class: "trajectory-legend",
                    span { class: "trajectory-key coast", i {} "Velocity / coast" }
                    span { class: "trajectory-key active", i {} "Held control input" }
                    span { class: "trajectory-key field", i {} "Prediction uncertainty" }
                }
            }
        }
    }
}

#[component]
fn QuickRadialMenu(gesture: QuickGesture, data: UiSnapshot) -> Element {
    if !gesture.active {
        return rsx! {};
    }
    let root_style = format!("left:{}px;top:{}px", gesture.origin.0, gesture.origin.1);
    let submenu_style = format!("left:{}px;top:{}px", gesture.submenu.0, gesture.submenu.1);
    rsx! {
        div { class: "quick-radial-gesture", aria_label: "Flight marking menu",
            div { class: "quick-wheel quick-root-wheel", style: "{root_style}",
                div { class: "quick-center", "DRAG" }
                for slot in 0..4 {
                    {
                        let tier = quick_tier_for_slot(slot);
                        let (symbol, label) = quick_tier_label(tier);
                        let selected = gesture.tier_slot == Some(slot);
                        rsx! {
                            div {
                                key: "tier-{slot}",
                                class: if selected { format!("quick-item root-slot-{slot} selected") } else { format!("quick-item root-slot-{slot}") },
                                b { "{symbol}" }
                                span { "{label}" }
                            }
                        }
                    }
                }
            }
            if let Some(tier) = gesture.tier {
                div { class: "quick-wheel quick-child-wheel", style: "{submenu_style}",
                    div { class: "quick-tier-label", "{quick_tier_label(tier).1}" }
                    div { class: "quick-center", "{quick_tier_label(tier).0}" }
                    for slot in 0..6 {
                        {
                            let (symbol, label, disabled) = quick_action_label(tier, slot, data);
                            let selected = gesture.action_slot == Some(slot) && !disabled;
                            let class = if selected {
                                format!("quick-item slot-{slot} selected")
                            } else {
                                format!("quick-item slot-{slot}")
                            };
                            rsx! {
                                div { key: "action-{slot}", class, aria_disabled: disabled,
                                    b { "{symbol}" }
                                    span { "{label}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn quick_tier_for_slot(slot: usize) -> QuickTier {
    match slot % 4 {
        0 => QuickTier::Camera,
        1 => QuickTier::Simulation,
        2 => QuickTier::Navigation,
        _ => QuickTier::Planner,
    }
}

fn quick_tier_label(tier: QuickTier) -> (&'static str, &'static str) {
    match tier {
        QuickTier::Camera => ("CAM", "CAMERA"),
        QuickTier::Simulation => ("SIM", "SIMULATION"),
        QuickTier::Navigation => ("NAV", "TRAJECTORY"),
        QuickTier::Planner => ("NODE", "MANEUVER"),
    }
}

fn quick_action_label(
    tier: QuickTier,
    slot: usize,
    data: UiSnapshot,
) -> (&'static str, &'static str, bool) {
    match (tier, slot) {
        (QuickTier::Camera, 0) => ("◎", "Center", false),
        (QuickTier::Camera, 1) => (
            if data.camera_follow { "FREE" } else { "LOCK" },
            if data.camera_follow {
                "Free cam"
            } else {
                "Follow"
            },
            false,
        ),
        (QuickTier::Camera, 2) => ("+", "Zoom in", false),
        (QuickTier::Camera, 3) => ("−", "Zoom out", false),
        (QuickTier::Camera, 4) => ("3×", "Default zoom", false),
        (QuickTier::Camera, 5) => ("CAN", "Canvas mode", false),
        (QuickTier::Simulation, 0) => ("Ⅱ", "Pause", false),
        (QuickTier::Simulation, 1) => ("½×", "Slow", false),
        (QuickTier::Simulation, 2) => ("1×", "Realtime", false),
        (QuickTier::Simulation, 3) => ("2×", "Fast", false),
        (QuickTier::Simulation, 4) => (
            "∞",
            if data.infinite_fuel {
                "Finite fuel"
            } else {
                "Infinite fuel"
            },
            false,
        ),
        (QuickTier::Simulation, 5) => ("G", "Gravity field", false),
        (QuickTier::Navigation, 0) => (
            "PATH",
            if data.trajectory {
                "Hide paths"
            } else {
                "Show paths"
            },
            false,
        ),
        (QuickTier::Navigation, 1) => ("×2", "Extend", false),
        (QuickTier::Navigation, 2) => ("÷2", "Shorten", false),
        (QuickTier::Navigation, 3) => ("FIELD", "Gravity field", false),
        (QuickTier::Navigation, 4) => ("NODE", "Planning mode", false),
        (QuickTier::Navigation, 5) => ("◎", "Recenter", false),
        (QuickTier::Planner, 0) => (
            "N",
            if data.maneuver_enabled {
                "Exit planning"
            } else {
                "Plan mode"
            },
            false,
        ),
        (QuickTier::Planner, 1) => (
            "+",
            "Add node",
            !data.maneuver_enabled || data.maneuver_armed,
        ),
        (QuickTier::Planner, 2) => (
            if data.maneuver_armed { "■" } else { "▶" },
            if data.maneuver_armed {
                "Abort burn"
            } else {
                "Execute"
            },
            !data.maneuver_enabled || data.maneuver_nodes == 0,
        ),
        (QuickTier::Planner, 3) => (
            "‹",
            "Previous node",
            data.maneuver_node_id == 0 || data.maneuver_armed,
        ),
        (QuickTier::Planner, 4) => (
            "›",
            "Next node",
            data.maneuver_node_id == 0 || data.maneuver_armed,
        ),
        (QuickTier::Planner, 5) => (
            "×",
            "Delete node",
            data.maneuver_node_id == 0 || data.maneuver_armed,
        ),
        _ => ("", "", true),
    }
}

fn radial_slot(dx: f64, dy: f64, count: usize) -> usize {
    let sector = std::f64::consts::TAU / count as f64;
    ((dy.atan2(dx) + std::f64::consts::FRAC_PI_2 + sector * 0.5).rem_euclid(std::f64::consts::TAU)
        / sector)
        .floor() as usize
        % count
}

fn update_quick_gesture(gesture: &mut QuickGesture) {
    if gesture.tier.is_none() {
        let dx = gesture.pointer.0 - gesture.origin.0;
        let dy = gesture.pointer.1 - gesture.origin.1;
        if dx.hypot(dy) >= 54.0 {
            let slot = radial_slot(dx, dy, 4);
            gesture.tier_slot = Some(slot);
            gesture.tier = Some(quick_tier_for_slot(slot));
            gesture.submenu = gesture.pointer;
        }
        return;
    }

    let dx = gesture.pointer.0 - gesture.submenu.0;
    let dy = gesture.pointer.1 - gesture.submenu.1;
    gesture.action_slot = (dx.hypot(dy) >= 42.0).then(|| radial_slot(dx, dy, 6));
}

fn execute_quick_action(
    tier: QuickTier,
    slot: usize,
    data: UiSnapshot,
    canvas_panelled: &mut Signal<bool>,
) {
    if quick_action_label(tier, slot, data).2 {
        return;
    }
    let horizon_up = data.trajectory_steps.saturating_mul(2).clamp(60, 360_000);
    let horizon_down = (data.trajectory_steps / 2).max(60);
    match (tier, slot) {
        (QuickTier::Camera, 0) => command(UiCommand::ResetCamera),
        (QuickTier::Camera, 1) => command(UiCommand::CameraFollow(!data.camera_follow)),
        (QuickTier::Camera, 2) => command(UiCommand::CameraZoom((data.camera_zoom / 1.6).max(0.3))),
        (QuickTier::Camera, 3) => {
            command(UiCommand::CameraZoom((data.camera_zoom * 1.6).min(900.0)))
        }
        (QuickTier::Camera, 4) => command(UiCommand::CameraZoom(3.0)),
        (QuickTier::Camera, 5) => {
            let next = !canvas_panelled();
            canvas_panelled.set(next);
            relocate_game_canvas(next);
        }
        (QuickTier::Simulation, 0) => command(UiCommand::Pause),
        (QuickTier::Simulation, 1) => command(UiCommand::TimeScale(0.5)),
        (QuickTier::Simulation, 2) => command(UiCommand::TimeScale(1.0)),
        (QuickTier::Simulation, 3) => command(UiCommand::TimeScale(2.0)),
        (QuickTier::Simulation, 4) => command(UiCommand::InfiniteFuel(!data.infinite_fuel)),
        (QuickTier::Simulation, 5) => command(UiCommand::GravityField(!data.gravity_field)),
        (QuickTier::Navigation, 0) => command(UiCommand::Trajectory(!data.trajectory)),
        (QuickTier::Navigation, 1) => command(UiCommand::TrajectorySteps(horizon_up)),
        (QuickTier::Navigation, 2) => command(UiCommand::TrajectorySteps(horizon_down)),
        (QuickTier::Navigation, 3) => command(UiCommand::GravityField(!data.gravity_field)),
        (QuickTier::Navigation, 4) => command(UiCommand::ManeuverMode(!data.maneuver_enabled)),
        (QuickTier::Navigation, 5) => command(UiCommand::ResetCamera),
        (QuickTier::Planner, 0) => command(UiCommand::ManeuverMode(!data.maneuver_enabled)),
        (QuickTier::Planner, 1) => command(UiCommand::ManeuverAdd),
        (QuickTier::Planner, 2) => command(UiCommand::ManeuverArm(!data.maneuver_armed)),
        (QuickTier::Planner, 3) => command(UiCommand::ManeuverSelectRelative(-1)),
        (QuickTier::Planner, 4) => command(UiCommand::ManeuverSelectRelative(1)),
        (QuickTier::Planner, 5) => command(UiCommand::ManeuverDeleteSelected),
        _ => {}
    }
}

fn panel_body(kind: GamePanel, data: UiSnapshot, canvas_panelled: Signal<bool>) -> Element {
    if kind == GamePanel::GameCanvas {
        return rsx! {
            div { class: "canvas-panel-shell",
                div { class: "canvas-panel-toolbar",
                    span { if canvas_panelled() { "Canvas hosted in panel" } else { "Canvas is fullscreen background" } }
                    span { "Resize this panel freely · dock/undock in Flight Controls" }
                }
                div { id: "canvas-panel-host", class: if canvas_panelled() { "canvas-panel-host active" } else { "canvas-panel-host" } }
            }
        };
    }

    match data.screen {
        UiScreen::Loading => rsx! { div { class: "game-panel", h1 { "Loading mission…" } } },
        UiScreen::Menu => match kind {
            GamePanel::FlightControls => rsx! {
                div { class: "game-panel menu-panel",
                    p { class: "eyebrow", "GERBIL AERONAUTICS DIRECTORATE" }
                    h1 { "SPACE PROGRAM" }
                    p { class: "muted", "Experimental orbital flight computer" }
                    button { onclick: move |_| command(UiCommand::Play), "Launch" }
                    button { class: "secondary", onclick: move |_| command(UiCommand::Settings), "Mission settings" }
                }
            },
            GamePanel::ShipDesigner => ship_designer_panel(data),
            _ => idle_panel(kind, "Awaiting mission launch"),
        },
        UiScreen::Settings => match kind {
            GamePanel::FlightControls => rsx! {
                div { class: "combined-panel",
                    {simulation_panel(data)}
                    div { class: "game-panel", button { onclick: move |_| command(UiCommand::Menu), "Back to mission control" } }
                }
            },
            GamePanel::ShipDesigner => ship_designer_panel(data),
            _ => idle_panel(kind, "Mission settings"),
        },
        UiScreen::Playing => match kind {
            GamePanel::Telemetry => telemetry_panel(data),
            GamePanel::FlightControls => combined_controls_panel(data, canvas_panelled),
            GamePanel::Minimap => rsx! { MinimapPanel { data } },
            GamePanel::ManeuverPlanner => maneuver_planner_panel(data),
            GamePanel::ShipDesigner => ship_designer_panel(data),
            GamePanel::GameCanvas => unreachable!(),
        },
        UiScreen::Paused => match kind {
            GamePanel::FlightControls => rsx! {
                div { class: "game-panel menu-panel",
                    p { class: "eyebrow", "MISSION HOLD" }
                    h1 { "Paused" }
                    button { onclick: move |_| command(UiCommand::Resume), "Resume" }
                    button { class: "secondary", onclick: move |_| command(UiCommand::ResetCamera), "Center camera" }
                    button { class: "secondary", onclick: move |_| command(UiCommand::Menu), "Abort to menu" }
                }
            },
            GamePanel::Minimap => rsx! { MinimapPanel { data } },
            GamePanel::ShipDesigner => ship_designer_panel(data),
            _ => idle_panel(kind, "Simulation paused"),
        },
        UiScreen::GameOver => match kind {
            GamePanel::Telemetry => rsx! {
                div { class: "game-panel menu-panel",
                    p { class: "eyebrow", "MISSION REPORT" }
                    h1 { "Flight ended" }
                    div { class: "final-score", "{data.score}" }
                    p { class: "muted", "Survived {data.time:.1} seconds" }
                    button { onclick: move |_| command(UiCommand::Play), "Fly again" }
                    button { class: "secondary", onclick: move |_| command(UiCommand::Menu), "Main menu" }
                }
            },
            GamePanel::Minimap => rsx! { MinimapPanel { data } },
            GamePanel::ShipDesigner => ship_designer_panel(data),
            _ => idle_panel(kind, "Mission complete"),
        },
    }
}

fn ship_designer_panel(data: UiSnapshot) -> Element {
    let rarity = ["Common", "Uncommon", "Rare", "Epic", "Legendary"]
        .get(data.ship_rarity as usize)
        .copied()
        .unwrap_or("Common");
    let manufacturer = [
        "Orion Dynamics",
        "Void Forge",
        "Solar Collective",
        "Rust Belt Customs",
        "Deep Space Mining Corp",
        "Xenotech Foundry",
    ]
    .get(data.ship_manufacturer as usize)
    .copied()
    .unwrap_or("Unknown yard");
    let archetype = [
        "Needle / Rocket",
        "Lifting Body",
        "Saucer",
        "Spherical Pod",
        "Ring Ship",
        "Dumbbell",
        "Spine / Truss",
        "Twin Boom",
        "Cargo Barge",
        "Modular Freighter",
        "Mining Rig",
        "Solar Sail",
        "Rotating Habitat",
        "Alien Crescent",
        "Swarm",
        "Armored Monitor",
    ]
    .get(data.ship_archetype as usize)
    .copied()
    .unwrap_or("Experimental");
    rsx! {
        div { class: "game-panel ship-designer",
            p { class: "eyebrow", "PROCEDURAL SHIPYARD" }
            div { class: "design-heading",
                strong { "{archetype} · {manufacturer}" }
                span { class: "rarity rarity-{data.ship_rarity}", "{rarity}" }
            }
            ShipPreview { data }
            label { "Design rarity" }
            select {
                value: "{data.ship_rarity}",
                onchange: move |event| if let Ok(rarity) = event.value().parse::<u8>() {
                    command(UiCommand::GenerateShip(rarity));
                },
                option { value: "0", "Common" }
                option { value: "1", "Uncommon" }
                option { value: "2", "Rare" }
                option { value: "3", "Epic" }
                option { value: "4", "Legendary" }
            }
            div { class: "metric-grid design-stats",
                Metric { label: "PARTS", value: data.ship_parts.to_string() }
                Metric { label: "HARDPOINTS", value: data.ship_hardpoints.to_string() }
                Metric { label: "THRUST", value: format!("{:.2}×", data.ship_thrust) }
                Metric { label: "MASS", value: format!("{:.2}×", data.ship_mass) }
                Metric { label: "ARMOR", value: format!("{:.2}×", data.ship_armor) }
                Metric { label: "HANDLING", value: format!("{:.2}×", data.ship_maneuverability) }
            }
            details { class: "flight-profile", open: true,
                summary { "Atmospheric flight-dynamics profile" }
                div { class: "profile-legend",
                    span { class: "com-key", "● COM" }
                    span { class: "cp-key", "○ CENTER OF PRESSURE" }
                }
                div { class: "metric-grid profile-stats",
                    Metric { label: "REF AREA", value: format!("{:.1}", data.ship_reference_area) }
                    Metric { label: "FRONTAL", value: format!("{:.1}", data.ship_frontal_area) }
                    Metric { label: "LATERAL", value: format!("{:.1}", data.ship_lateral_area) }
                    Metric { label: "TOP", value: format!("{:.1}", data.ship_top_area) }
                    Metric { label: "CD X/Y/Z", value: format!("{:.2} / {:.2} / {:.2}", data.ship_drag_coefficients[0], data.ship_drag_coefficients[1], data.ship_drag_coefficients[2]) }
                    Metric { label: "LIFT SLOPE", value: format!("{:.2}/rad", data.ship_lift_slope) }
                    Metric { label: "STALL", value: format!("{:.1}°", data.ship_stall_angle.to_degrees()) }
                    Metric { label: "YAW INERTIA", value: format!("{:.1}", data.ship_inertia[2]) }
                    Metric { label: "JOINTS", value: data.ship_joint_count.to_string() }
                }
            }
            details { class: "parts-manifest",
                summary { "Generated module manifest" }
                for index in 0..(data.ship_parts as usize).min(8) {
                    div { class: "part-row",
                        span { {part_slot_name(data.ship_part_slots[index])} }
                        strong { {manufacturer_name(data.ship_part_manufacturers[index])} }
                        em { class: "rarity rarity-{data.ship_part_rarities[index]}", {rarity_name(data.ship_part_rarities[index])} }
                    }
                }
            }
            button { onclick: move |_| command(UiCommand::GenerateShip(data.ship_rarity)), "Reroll design" }
            p { class: "design-seed", "Blueprint seed · {data.ship_seed:016X}" }
        }
    }
}

fn part_slot_name(slot: u8) -> &'static str {
    [
        "Hull",
        "Drive",
        "Reactor",
        "Cockpit",
        "Wings",
        "Stabilizer",
        "Special",
    ]
    .get(slot as usize)
    .copied()
    .unwrap_or("Module")
}

fn manufacturer_name(manufacturer: u8) -> &'static str {
    [
        "Orion",
        "Void Forge",
        "Solar",
        "Rust Belt",
        "Deep Space",
        "Xenotech",
    ]
    .get(manufacturer as usize)
    .copied()
    .unwrap_or("Unknown")
}

fn rarity_name(rarity: u8) -> &'static str {
    ["Common", "Uncommon", "Rare", "Epic", "Legendary"]
        .get(rarity as usize)
        .copied()
        .unwrap_or("Common")
}

#[component]
fn ShipPreview(data: UiSnapshot) -> Element {
    let count = data.ship_module_count as usize;
    let modules = &data.ship_modules[..count];
    let extent = modules.iter().fold(35.0_f32, |extent, module| {
        extent
            .max(module.x.abs() + module.width * 0.65)
            .max(module.y.abs() + module.length * 0.65)
    }) * 1.18;
    let mut isometric_modules = modules.to_vec();
    isometric_modules.sort_by(|a, b| (a.x + a.y + a.z).total_cmp(&(b.x + b.y + b.z)));
    rsx! {
        div { class: "ship-preview-pair",
            div { class: "projection-preview",
                span { class: "projection-label", "2D · TOP" }
                svg { class: "ship-preview", view_box: "{-extent} {-extent} {extent * 2.0} {extent * 2.0}",
            line { class: "preview-axis", x1: "{-extent}", y1: "0", x2: "{extent}", y2: "0" }
            line { class: "preview-axis", x1: "0", y1: "{-extent}", x2: "0", y2: "{extent}" }
            g { fill: "none", stroke_width: "1.8", vector_effect: "non-scaling-stroke",
                for module in modules.iter() {
                    g { stroke: "{ship_layer_color(data.ship_palette, module.color_layer)}", transform: "rotate({top_projection_angle(module.rotation)} {module.x} {-module.y})",
                        match module.primitive {
                            0 | 5 | 6 => rsx! {
                                rect { x: "{module.x - module.width * 0.5}", y: "{-module.y - module.length * 0.5}", width: "{module.width}", height: "{module.length}" }
                                if module.primitive == 5 {
                                    line { x1: "{module.x - module.width * 0.5}", y1: "{-module.y - module.length * 0.5}", x2: "{module.x + module.width * 0.5}", y2: "{-module.y + module.length * 0.5}" }
                                    line { x1: "{module.x + module.width * 0.5}", y1: "{-module.y - module.length * 0.5}", x2: "{module.x - module.width * 0.5}", y2: "{-module.y + module.length * 0.5}" }
                                }
                            },
                            1 => rsx! { polygon { points: "{module.x},{-module.y - module.length * 0.5} {module.x - module.width * 0.5},{-module.y + module.length * 0.5} {module.x + module.width * 0.5},{-module.y + module.length * 0.5}" } },
                            2 | 3 => rsx! { ellipse { cx: "{module.x}", cy: "{-module.y}", rx: "{module.width * 0.5}", ry: "{module.length * 0.5}" } },
                            4 => rsx! {
                                ellipse { cx: "{module.x}", cy: "{-module.y}", rx: "{module.width * 0.5}", ry: "{module.length * 0.5}" }
                                ellipse { cx: "{module.x}", cy: "{-module.y}", rx: "{module.width * 0.29}", ry: "{module.length * 0.29}", stroke_opacity: "0.72" }
                            },
                            _ => rsx! { path { d: "M {module.x - module.width * 0.42} {-module.y - module.length * 0.42} A {module.width * 0.5} {module.length * 0.5} 0 1 0 {module.x - module.width * 0.42} {-module.y + module.length * 0.42} A {module.width * 0.28} {module.length * 0.28} 0 0 1 {module.x + module.width * 0.08} {-module.y - module.length * 0.18}" } },
                        }
                    }
                }
                circle { class: "mass-center", cx: "{data.ship_center_of_mass[0]}", cy: "{-data.ship_center_of_mass[1]}", r: "1.8" }
                circle { class: "pressure-center", cx: "{data.ship_center_of_pressure[0]}", cy: "{-data.ship_center_of_pressure[1]}", r: "2.4" }
            }
                }
            }
            div { class: "projection-preview",
                span { class: "projection-label", "3D · ISOMETRIC" }
                svg { class: "ship-preview ship-preview-3d", view_box: "{-extent * 1.2} {-extent * 1.2} {extent * 2.4} {extent * 2.4}",
                    line { class: "preview-axis", x1: "{-extent}", y1: "0", x2: "{extent}", y2: "0" }
                    line { class: "preview-axis", x1: "0", y1: "{-extent}", x2: "0", y2: "{extent}" }
                    for module in isometric_modules {
                        IsoModule { module, palette: data.ship_palette }
                    }
                    {
                        let (x, y) = iso_project(data.ship_center_of_mass);
                        rsx! { circle { class: "mass-center", cx: "{x}", cy: "{y}", r: "1.8" } }
                    }
                    {
                        let (x, y) = iso_project(data.ship_center_of_pressure);
                        rsx! { circle { class: "pressure-center", cx: "{x}", cy: "{y}", r: "2.4" } }
                    }
                }
            }
        }
    }
}

fn iso_project(point: [f32; 3]) -> (f32, f32) {
    let [x, y, z] = point;
    ((x - y) * 0.866_025_4, (x + y) * 0.5 - z)
}

fn rotate_preview_vector(vector: [f32; 3], quaternion: [f32; 4]) -> [f32; 3] {
    let [x, y, z] = vector;
    let [qx, qy, qz, qw] = quaternion;
    let tx = 2.0 * (qy * z - qz * y);
    let ty = 2.0 * (qz * x - qx * z);
    let tz = 2.0 * (qx * y - qy * x);
    [
        x + qw * tx + qy * tz - qz * ty,
        y + qw * ty + qz * tx - qx * tz,
        z + qw * tz + qx * ty - qy * tx,
    ]
}

fn top_projection_angle(rotation: [f32; 4]) -> f32 {
    let forward = rotate_preview_vector([0.0, 1.0, 0.0], rotation);
    -forward[0].atan2(forward[1]).to_degrees()
}

fn svg_points(points: &[(f32, f32)]) -> String {
    points
        .iter()
        .map(|(x, y)| format!("{x:.2},{y:.2}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[component]
fn IsoModule(module: crate::ui::bridge::UiShipModule, palette: u8) -> Element {
    let color = ship_layer_color(palette, module.color_layer);
    let half = [module.width * 0.5, module.length * 0.5, module.height * 0.5];
    let corner = |x: f32, y: f32, z: f32| {
        let rotated = rotate_preview_vector([x, y, z], module.rotation);
        iso_project([
            module.x + rotated[0],
            module.y + rotated[1],
            module.z + rotated[2],
        ])
    };
    let p000 = corner(-half[0], -half[1], -half[2]);
    let p100 = corner(half[0], -half[1], -half[2]);
    let p010 = corner(-half[0], half[1], -half[2]);
    let p110 = corner(half[0], half[1], -half[2]);
    let p001 = corner(-half[0], -half[1], half[2]);
    let p101 = corner(half[0], -half[1], half[2]);
    let p011 = corner(-half[0], half[1], half[2]);
    let p111 = corner(half[0], half[1], half[2]);
    let top = svg_points(&[p001, p101, p111, p011]);
    let left = svg_points(&[p000, p001, p011, p010]);
    let right = svg_points(&[p100, p101, p111, p110]);
    let (cx, cy) = iso_project([module.x, module.y, module.z]);
    let ellipse_rx = module.width.max(module.length) * 0.43;
    let ellipse_ry = (module.width.max(module.length) * 0.20 + module.height * 0.25).max(1.2);
    rsx! {
        g { class: "iso-module", stroke: "{color}", stroke_width: "1.2", vector_effect: "non-scaling-stroke",
            if module.primitive == 3 {
                ellipse { cx: "{cx}", cy: "{cy}", rx: "{ellipse_rx}", ry: "{ellipse_ry}", fill: "{color}", fill_opacity: ".24" }
                path { d: "M {cx - ellipse_rx} {cy} Q {cx} {cy + ellipse_ry * 0.75} {cx + ellipse_rx} {cy}", fill: "none", stroke_opacity: ".55" }
            } else if module.primitive == 2 || module.primitive == 4 {
                ellipse { cx: "{cx}", cy: "{cy - half[2]}", rx: "{ellipse_rx}", ry: "{ellipse_ry * 0.55}", fill: "{color}", fill_opacity: ".20" }
                line { x1: "{cx - ellipse_rx}", y1: "{cy - half[2]}", x2: "{cx - ellipse_rx}", y2: "{cy + half[2]}" }
                line { x1: "{cx + ellipse_rx}", y1: "{cy - half[2]}", x2: "{cx + ellipse_rx}", y2: "{cy + half[2]}" }
                ellipse { cx: "{cx}", cy: "{cy + half[2]}", rx: "{ellipse_rx}", ry: "{ellipse_ry * 0.55}", fill: "{color}", fill_opacity: ".10" }
                if module.primitive == 4 {
                    ellipse { cx: "{cx}", cy: "{cy - half[2]}", rx: "{ellipse_rx * 0.56}", ry: "{ellipse_ry * 0.30}", fill: "var(--bg)", fill_opacity: ".88" }
                }
            } else {
                polygon { points: "{left}", fill: "{color}", fill_opacity: ".10" }
                polygon { points: "{right}", fill: "{color}", fill_opacity: ".18" }
                polygon { points: "{top}", fill: "{color}", fill_opacity: ".28" }
                if module.primitive == 5 {
                    line { x1: "{p001.0}", y1: "{p001.1}", x2: "{p111.0}", y2: "{p111.1}", stroke_opacity: ".6" }
                    line { x1: "{p101.0}", y1: "{p101.1}", x2: "{p011.0}", y2: "{p011.1}", stroke_opacity: ".6" }
                }
            }
        }
    }
}

fn ship_layer_color(palette: u8, layer: u8) -> &'static str {
    const COLORS: [[&str; 3]; 6] = [
        ["#b8dcff", "#426b94", "#67ebff"],
        ["#9458e0", "#382057", "#e96bff"],
        ["#f5c738", "#664513", "#fff092"],
        ["#e06e2e", "#574030", "#73ef9e"],
        ["#adb09c", "#454a45", "#ffad33"],
        ["#2ef0a8", "#145047", "#adffe1"],
    ];
    COLORS[(palette / 3).min(5) as usize][layer.min(2) as usize]
}

#[allow(dead_code)]
fn LegacyShipPreview(data: UiSnapshot) -> Element {
    let color = match data.ship_rarity {
        1 => "#72dc88",
        2 => "#65a9ff",
        3 => "#c47aff",
        4 => "#ffc45e",
        _ => "#d6dde8",
    };
    let half_width = data.ship_hull_width * 0.5;
    let half_length = data.ship_hull_length * 0.5;
    let hull_points = match data.ship_hull_variant % 3 {
        0 => format!(
            "0,{} {},{} {},{} {},{} {},{}",
            -half_length,
            -half_width,
            -half_length * 0.15,
            -half_width * 0.72,
            half_length,
            half_width * 0.72,
            half_length,
            half_width,
            -half_length * 0.15
        ),
        1 => format!(
            "0,{} {},{} {},{} {},{} {},{} {},{}",
            -half_length,
            -half_width * 0.68,
            -half_length * 0.48,
            -half_width,
            half_length * 0.45,
            -half_width * 0.45,
            half_length,
            half_width * 0.45,
            half_length,
            half_width,
            half_length * 0.45
        ),
        _ => format!(
            "0,{} {},{} {},{} {},{} {},{} {},{}",
            -half_length,
            -half_width * 0.45,
            -half_length * 0.55,
            -half_width,
            -half_length * 0.05,
            -half_width * 0.55,
            half_length,
            half_width * 0.55,
            half_length,
            half_width,
            -half_length * 0.05
        ),
    };
    let wing_sweep = -data.ship_wing_sweep * half_length;
    let left_wing = format!(
        "{},{} {},{} {},{} {},{}",
        -half_width * 0.75,
        -half_length * 0.30,
        -data.ship_wing_span,
        wing_sweep,
        -data.ship_wing_span * 0.78,
        wing_sweep + half_length * 0.24,
        -half_width * 0.62,
        half_length * 0.42
    );
    let right_wing = format!(
        "{},{} {},{} {},{} {},{}",
        half_width * 0.75,
        -half_length * 0.30,
        data.ship_wing_span,
        wing_sweep,
        data.ship_wing_span * 0.78,
        wing_sweep + half_length * 0.24,
        half_width * 0.62,
        half_length * 0.42
    );
    let engine_count = data.ship_engine_count.max(1);
    let engine_xs: Vec<f32> = (0..engine_count)
        .map(|index| {
            if engine_count == 1 {
                0.0
            } else {
                (index as f32 / (engine_count - 1) as f32 - 0.5)
                    * half_width
                    * data.ship_engine_spread
                    * 2.0
            }
        })
        .collect();
    rsx! {
        svg { class: "ship-preview", view_box: "-75 -50 150 100",
            line { class: "preview-axis", x1: "-68", y1: "0", x2: "68", y2: "0" }
            line { class: "preview-axis", x1: "0", y1: "-44", x2: "0", y2: "44" }
            g { fill: "none", stroke: "{color}", stroke_width: "2.2", vector_effect: "non-scaling-stroke",
                polygon { points: "{left_wing}" }
                polygon { points: "{right_wing}" }
                if data.ship_wing_pairs > 1 {
                    polygon { points: "{-half_width * 0.68},{half_length * 0.12} {-data.ship_wing_span * 0.72},{wing_sweep + half_length * 0.38} {-data.ship_wing_span * 0.54},{wing_sweep + data.ship_wing_chord * 0.65} {-half_width * 0.55},{half_length * 0.42}" }
                    polygon { points: "{half_width * 0.68},{half_length * 0.12} {data.ship_wing_span * 0.72},{wing_sweep + half_length * 0.38} {data.ship_wing_span * 0.54},{wing_sweep + data.ship_wing_chord * 0.65} {half_width * 0.55},{half_length * 0.42}" }
                }
                polygon { points: "{hull_points}" }
                for index in 1..data.ship_section_count.clamp(2, 4) {
                    line {
                        x1: "{-data.ship_hull_sections[index as usize] * 0.5}",
                        x2: "{data.ship_hull_sections[index as usize] * 0.5}",
                        y1: "{-half_length + index as f32 / data.ship_section_count as f32 * data.ship_hull_length}",
                        y2: "{-half_length + index as f32 / data.ship_section_count as f32 * data.ship_hull_length}",
                        stroke_opacity: "0.7",
                    }
                }
                match data.ship_cockpit_variant % 3 {
                    0 => rsx! { circle { cx: "0", cy: "{-half_length * 0.28}", r: "{half_width * 0.34}", stroke: "#73d8ff" } },
                    1 => rsx! { polygon { points: "0,{-half_length * 0.53} {-half_width * 0.4},{-half_length * 0.16} {half_width * 0.4},{-half_length * 0.16}", stroke: "#73d8ff" } },
                    _ => rsx! { rect { x: "{-half_width * 0.4}", y: "{-half_length * 0.38}", width: "{half_width * 0.8}", height: "9", stroke: "#73d8ff" } },
                }
                for x in engine_xs {
                    rect { x: "{x - 3.5}", y: "{half_length * 0.70}", width: "7", height: "{data.ship_engine_length}" }
                }
                if data.ship_special_variant != 0 {
                    circle { cx: "0", cy: "0", r: "{half_width * 0.36}", stroke_dasharray: "2 2" }
                }
                for plate in 0..(1 + data.ship_armor_variant % 4) {
                    rect {
                        x: "{-half_width * (0.34 + plate as f32 * 0.04)}",
                        y: "{-2.0 + plate as f32 * 6.0}",
                        width: "{half_width * (0.68 + plate as f32 * 0.08)}",
                        height: "3",
                        stroke_opacity: "0.55",
                    }
                }
                for scratch in 0..data.ship_wear.min(5) {
                    line { x1: "{-half_width * 0.6 + scratch as f32 * 4.0}", y1: "{scratch as f32 % 2.0 * 5.0}", x2: "{-half_width * 0.6 + scratch as f32 * 4.0 + 3.0}", y2: "{scratch as f32 % 2.0 * 5.0 + 2.0}", stroke_opacity: "0.45" }
                }
            }
        }
    }
}

fn telemetry_panel(data: UiSnapshot) -> Element {
    rsx! {
        div { class: "game-panel hud",
                p { class: "eyebrow", "FLIGHT TELEMETRY" }
                div { class: "metric-grid",
                    Metric { label: "TIME", value: format!("{:.1}s", data.time) }
                    Metric { label: "SCORE", value: data.score.to_string() }
                    Metric { label: "FUEL", value: if data.infinite_fuel { "∞".into() } else { format!("{:.1}", data.fuel) } }
                    Metric { label: "VELOCITY", value: format!("{:.1}, {:.1}", data.velocity_x, data.velocity_y) }
                    Metric { label: "ROTATION", value: format!("{:.2}", data.angular_velocity) }
                    Metric { label: "TIME SCALE", value: format!("{:.1}×", data.time_scale) }
                    Metric { label: "FPS", value: if data.fps > 0.0 { format!("{:.0}", data.fps) } else { "—".into() } }
                    Metric { label: "FRAME", value: if data.frame_time_ms > 0.0 { format!("{:.2} ms", data.frame_time_ms) } else { "—".into() } }
                }
        }
    }
}

fn combined_controls_panel(data: UiSnapshot, canvas_panelled: Signal<bool>) -> Element {
    rsx! {
        div { class: "combined-panel",
            {flight_controls_panel(data, canvas_panelled)}
            {simulation_panel(data)}
        }
    }
}

#[component]
fn MinimapPanel(data: UiSnapshot) -> Element {
    let mut pan = use_signal(|| (0.0_f32, 0.0_f32));
    let mut dragging = use_signal(|| false);
    let count = data.map_body_count as usize;
    let bodies = &data.map_bodies[..count];
    let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
    let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for body in bodies {
        min_x = min_x.min(body.x - body.radius);
        min_y = min_y.min(body.y - body.radius);
        max_x = max_x.max(body.x + body.radius);
        max_y = max_y.max(body.y + body.radius);
    }
    min_x = min_x.min(data.ship_x);
    min_y = min_y.min(data.ship_y);
    max_x = max_x.max(data.ship_x);
    max_y = max_y.max(data.ship_y);
    if !min_x.is_finite() || !min_y.is_finite() || !max_x.is_finite() || !max_y.is_finite() {
        (min_x, min_y, max_x, max_y) = (-1.0, -1.0, 1.0, 1.0);
    }
    let width = (max_x - min_x).max(100.0) * 1.15;
    let height = (max_y - min_y).max(100.0) * 1.15;
    let fitted_center = ((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
    let view_box = format!(
        "{} {} {} {}",
        fitted_center.0 + pan().0 - width * 0.5,
        fitted_center.1 + pan().1 - height * 0.5,
        width,
        height,
    );
    let marker_radius = width.max(height) * 0.009;

    rsx! {
        div { class: "game-panel map-panel",
            p { class: "eyebrow", "SYSTEM NAVIGATION" }
            svg {
                class: if dragging() { "system-map dragging" } else { "system-map" },
                view_box: "{view_box}",
                preserve_aspect_ratio: "none",
                onmousedown: move |event| {
                    event.stop_propagation();
                    dragging.set(true);
                },
                onmousemove: move |event| {
                    event.stop_propagation();
                    if !dragging() {
                        return;
                    }
                    if let Some(mouse) = event.data().downcast::<web_sys::MouseEvent>() {
                        if let Some(element) = mouse
                            .current_target()
                            .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                        {
                            let pixel_width = element.client_width().max(1) as f32;
                            let pixel_height = element.client_height().max(1) as f32;
                            let (x, y) = pan();
                            pan.set((
                                x - mouse.movement_x() as f32 * width / pixel_width,
                                y - mouse.movement_y() as f32 * height / pixel_height,
                            ));
                        }
                    }
                },
                onmouseup: move |event| {
                    event.stop_propagation();
                    dragging.set(false);
                },
                onmouseleave: move |_| dragging.set(false),
                for body in bodies.iter() {
                    if body.parent >= 0 {
                        circle {
                            class: "map-orbit",
                            cx: "{data.map_bodies[body.parent as usize].x}",
                            cy: "{data.map_bodies[body.parent as usize].y}",
                            r: "{body.orbit_radius}",
                        }
                    }
                }
                for body in bodies.iter() {
                    circle {
                        class: "map-body",
                        cx: "{body.x}", cy: "{body.y}",
                        r: "{body.radius.max(marker_radius)}",
                    }
                }
                circle { class: "map-ship", cx: "{data.ship_x}", cy: "{data.ship_y}", r: "{marker_radius * 0.72}" }
            }
            div { class: "map-footer",
                p { class: "map-legend", "Drag map to pan · ● bodies · △ ship" }
                button {
                    class: "secondary compact",
                    onmousedown: move |event| event.stop_propagation(),
                    onclick: move |event| {
                        event.stop_propagation();
                        pan.set((0.0, 0.0));
                    },
                    "Fit system"
                }
            }
        }
    }
}

fn flight_controls_panel(data: UiSnapshot, mut canvas_panelled: Signal<bool>) -> Element {
    rsx! {
        div { class: "game-panel hud",
            p { class: "eyebrow", "FLIGHT CONTROLS" }
            div { class: "flight-controls",
                    label { class: "check", input { r#type: "checkbox", checked: data.camera_follow,
                        onchange: move |e| command(UiCommand::CameraFollow(e.checked())) } "Follow lander" }
                    label { class: "check", input { r#type: "checkbox", checked: data.gravity_field,
                        onchange: move |e| command(UiCommand::GravityField(e.checked())) }
                        "Gravity field (inertial; low-force markers are not rotating-frame Lagrange solutions)" }
                    label { "Camera zoom  {data.camera_zoom:.1}×" }
                    input { r#type: "range", min: "0.3", max: "900", step: "0.1", value: "{data.camera_zoom}",
                        oninput: move |e| if let Ok(value) = e.value().parse() { command(UiCommand::CameraZoom(value)); }
                    }
                    div { class: "button-row",
                        button { onclick: move |_| command(UiCommand::Pause), "Pause" }
                        button { class: "secondary", onclick: move |_| command(UiCommand::ResetCamera), "Center camera" }
                    }
                    div { class: "button-row",
                        button { class: "secondary", onclick: move |_| command(UiCommand::ResetFlight), "Reset flight" }
                        button { class: "danger", onclick: move |_| command(UiCommand::Menu), "Abort to menu" }
                    }
                    button { class: "secondary",
                        onmousedown: move |event| event.stop_propagation(),
                        onclick: move |event| {
                            event.stop_propagation();
                            let next = !canvas_panelled();
                            canvas_panelled.set(next);
                            relocate_game_canvas(next);
                        },
                        if canvas_panelled() { "Canvas: panel (switch fullscreen)" } else { "Canvas: fullscreen (dock in panel)" }
                    }
                }
                details { class: "key-reference",
                    summary { "Keyboard & mouse controls" }
                    dl {
                        dt { "Flight" } dd { "W / ↑ main thrust · S / ↓ reverse · A / D strafe · ← / → rotate · middle-drag quick menu" }
                        dt { "Camera" } dd { "+ / − zoom · R center/follow · F toggle follow · M minimap · drag when follow is off" }
                        dt { "Simulation" } dd { "G / H gravity ± · T / Y thrust ± · U / J time ±" }
                        dt { "Prediction" } dd { "P trajectory · [ / ] samples · I infinite fuel · V gravity field" }
                        dt { "Planner" } dd { "N mode · K add node · Enter execute/abort · Backspace clear" }
                        dt { "Mission" } dd { "Esc pause/resume" }
                    }
                }
        }
    }
}

fn maneuver_planner_panel(data: UiSnapshot) -> Element {
    rsx! {
        div { class: "game-panel maneuver-planner",
            p { class: "eyebrow", "ORBITAL OPERATIONS" }
            label { class: "check",
                input { r#type: "checkbox", checked: data.maneuver_enabled,
                    onchange: move |e| command(UiCommand::ManeuverMode(e.checked())) }
                "Planning mode (N)"
            }
            if data.maneuver_enabled {
                div { class: "timeline-status",
                    "T+{data.maneuver_elapsed:.1}s · {data.maneuver_nodes} node(s) · "
                    if data.maneuver_armed { "EXECUTING" } else { "SAFE / EDITING" }
                }
                div { class: "button-row",
                    button {
                        class: if data.maneuver_armed { "danger" } else { "secondary" },
                        onclick: move |_| command(UiCommand::ManeuverArm(!data.maneuver_armed)),
                        if data.maneuver_armed { "Abort" } else { "Arm & execute" }
                    }
                    button { class: "secondary", disabled: data.maneuver_armed,
                        onclick: move |_| command(UiCommand::ManeuverAdd), "Add node" }
                }
                if data.maneuver_node_id != 0 {
                    div { class: "node-pager",
                        button { class: "secondary compact", disabled: data.maneuver_armed,
                            onclick: move |_| command(UiCommand::ManeuverSelectRelative(-1)), "‹ PREV" }
                        strong { "NODE {data.maneuver_node_id}" }
                        button { class: "secondary compact", disabled: data.maneuver_armed,
                            onclick: move |_| command(UiCommand::ManeuverSelectRelative(1)), "NEXT ›" }
                    }
                    ManeuverRadialEditor { data }
                    details { class: "maneuver-precision",
                        summary { "Precision values" }
                        ManeuverRange { data, label: "Node time", field: "at", min: 0.0, max: 300.0, step: 0.25, value: data.maneuver_at }
                        ManeuverRange { data, label: "Duration", field: "duration", min: 0.1, max: 60.0, step: 0.1, value: data.maneuver_duration }
                        ManeuverRange { data, label: "Prograde / retrograde", field: "prograde", min: -1.0, max: 1.0, step: 0.01, value: data.maneuver_prograde }
                        ManeuverRange { data, label: "Radial in / out", field: "radial", min: -1.0, max: 1.0, step: 0.01, value: data.maneuver_radial }
                        ManeuverRange { data, label: "Attitude trim", field: "rotation", min: -1.0, max: 1.0, step: 0.01, value: data.maneuver_rotation }
                        ManeuverRange { data, label: "Throttle", field: "throttle", min: 0.0, max: 1.0, step: 0.01, value: data.maneuver_throttle }
                    }
                    button { class: "danger", disabled: data.maneuver_armed,
                        onclick: move |_| command(UiCommand::ManeuverDeleteSelected), "Delete selected node" }
                }
                button { class: "danger", disabled: data.maneuver_armed,
                    onclick: move |_| command(UiCommand::ManeuverClear), "Clear timeline" }
            } else {
                p { class: "muted", "Enable planning to preview and schedule burns without touching the live controls." }
            }
        }
    }
}

fn adjust_maneuver(
    data: UiSnapshot,
    at: f32,
    duration: f32,
    prograde: f32,
    radial: f32,
    rotation: f32,
    throttle: f32,
) {
    command(UiCommand::ManeuverEdit {
        id: data.maneuver_node_id,
        at,
        duration,
        prograde,
        radial,
        rotation,
        throttle,
    });
}

#[component]
fn ManeuverRadialEditor(data: UiSnapshot) -> Element {
    let vector_magnitude = (data.maneuver_prograde * data.maneuver_prograde
        + data.maneuver_radial * data.maneuver_radial)
        .sqrt();
    let disabled = data.maneuver_armed;
    rsx! {
        div { class: "maneuver-gizmo",
            div { class: "maneuver-orbit-ring" }
            button { class: "maneuver-handle prograde", disabled,
                title: "Increase prograde burn",
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration, data.maneuver_prograde + 0.05, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle),
                b { "P+" } span { "prograde" }
            }
            button { class: "maneuver-handle retrograde", disabled,
                title: "Increase retrograde burn",
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration, data.maneuver_prograde - 0.05, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle),
                b { "P−" } span { "retrograde" }
            }
            button { class: "maneuver-handle radial-out", disabled,
                title: "Increase radial-out burn",
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial + 0.05, data.maneuver_rotation, data.maneuver_throttle),
                b { "R+" } span { "radial out" }
            }
            button { class: "maneuver-handle radial-in", disabled,
                title: "Increase radial-in burn",
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial - 0.05, data.maneuver_rotation, data.maneuver_throttle),
                b { "R−" } span { "radial in" }
            }
            button { class: "maneuver-time earlier", disabled,
                title: "Move node earlier along orbit",
                onclick: move |_| adjust_maneuver(data, data.maneuver_at - 1.0, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle), "−1s" }
            button { class: "maneuver-time later", disabled,
                title: "Move node later along orbit",
                onclick: move |_| adjust_maneuver(data, data.maneuver_at + 1.0, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle), "+1s" }
            div { class: "maneuver-center",
                small { "T+{data.maneuver_at:.1}" }
                strong { "Δ {vector_magnitude:.2}" }
                span { "{data.maneuver_throttle * 100.0:.0}% · {data.maneuver_duration:.1}s" }
            }
        }
        div { class: "maneuver-fine-ring",
            button { class: "secondary compact", disabled,
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration - 0.5, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle), "DUR −" }
            button { class: "secondary compact", disabled,
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration + 0.5, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle), "DUR +" }
            button { class: "secondary compact", disabled,
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation - 0.05, data.maneuver_throttle), "TRIM ↺" }
            button { class: "secondary compact", disabled,
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation + 0.05, data.maneuver_throttle), "TRIM ↻" }
            button { class: "secondary compact", disabled,
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle - 0.05), "THR −" }
            button { class: "secondary compact", disabled,
                onclick: move |_| adjust_maneuver(data, data.maneuver_at, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle + 0.05), "THR +" }
        }
    }
}

#[component]
fn ManeuverRange(
    data: UiSnapshot,
    label: &'static str,
    field: &'static str,
    min: f32,
    max: f32,
    step: f32,
    value: f32,
) -> Element {
    rsx! {
        label { "{label}  {value:.2}" }
        input { r#type: "range", min: "{min}", max: "{max}", step: "{step}", value: "{value}",
            disabled: data.maneuver_armed,
            oninput: move |event| if let Ok(next) = event.value().parse::<f32>() {
                let (at, duration, prograde, radial, rotation, throttle) = match field {
                    "at" => (next, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle),
                    "duration" => (data.maneuver_at, next, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle),
                    "prograde" => (data.maneuver_at, data.maneuver_duration, next, data.maneuver_radial, data.maneuver_rotation, data.maneuver_throttle),
                    "radial" => (data.maneuver_at, data.maneuver_duration, data.maneuver_prograde, next, data.maneuver_rotation, data.maneuver_throttle),
                    "rotation" => (data.maneuver_at, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial, next, data.maneuver_throttle),
                    _ => (data.maneuver_at, data.maneuver_duration, data.maneuver_prograde, data.maneuver_radial, data.maneuver_rotation, next),
                };
                command(UiCommand::ManeuverEdit { id: data.maneuver_node_id, at, duration, prograde, radial, rotation, throttle });
            }
        }
    }
}

fn simulation_panel(data: UiSnapshot) -> Element {
    rsx! {
        div { class: "game-panel",
            p { class: "eyebrow", "SIMULATION" }
            label { "Gravity  {data.gravity:.2}×" }
            input { r#type: "range", min: "0.1", max: "3", step: "0.1", value: "{data.gravity}",
                oninput: move |e| if let Ok(value) = e.value().parse() { command(UiCommand::Gravity(value)); }
            }
            label { "Thrust  {data.thrust:.2}×" }
            input { r#type: "range", min: "0.1", max: "3", step: "0.1", value: "{data.thrust}",
                oninput: move |e| if let Ok(value) = e.value().parse() { command(UiCommand::Thrust(value)); }
            }
            label { "Time  {data.time_scale:.2}×" }
            input { r#type: "range", min: "0.1", max: "500", step: "0.1", value: "{data.time_scale}",
                oninput: move |e| if let Ok(value) = e.value().parse() { command(UiCommand::TimeScale(value)); }
            }
            label { class: "check", input { r#type: "checkbox", checked: data.trajectory,
                onchange: move |e| command(UiCommand::Trajectory(e.checked())) } "Show trajectories" }
            label { "Prediction  {data.trajectory_steps / 60}s" }
            input { r#type: "range", min: "60", max: "360000", step: "60", value: "{data.trajectory_steps}",
                oninput: move |e| if let Ok(value) = e.value().parse() { command(UiCommand::TrajectorySteps(value)); }
            }
            label { class: "check", input { r#type: "checkbox", checked: data.infinite_fuel,
                onchange: move |e| command(UiCommand::InfiniteFuel(e.checked())) } "Infinite fuel" }
        }
    }
}

fn idle_panel(kind: GamePanel, status: &'static str) -> Element {
    rsx! { div { class: "game-panel", p { class: "eyebrow", "{kind.title()}" } p { class: "muted", "{status}" } } }
}

fn relocate_game_canvas(panelled: bool) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(canvas) = document.get_element_by_id("bevy") else {
        return;
    };
    let target = if panelled {
        document.get_element_by_id("canvas-panel-host")
    } else {
        document.query_selector(".game-container").ok().flatten()
    };
    if let Some(target) = target {
        let _ = target.append_child(&canvas);
        if let Some(container) = document.query_selector(".game-container").ok().flatten() {
            container.set_class_name(if panelled {
                "game-container panelled"
            } else {
                "game-container"
            });
        }
        if let Some(html) = canvas.dyn_ref::<web_sys::HtmlElement>() {
            let _ = html.focus();
        }
        dispatch_canvas_resize();
    }
}

fn canvas_panel_size() -> Option<(i32, i32)> {
    let document = web_sys::window()?.document()?;
    let host = document.get_element_by_id("canvas-panel-host")?;
    Some((host.client_width(), host.client_height()))
}

fn dispatch_canvas_resize() {
    let Some(window) = web_sys::window() else {
        return;
    };
    if let Ok(event) = web_sys::Event::new("resize") {
        let _ = window.dispatch_event(&event);
    }
}

#[component]
fn Metric(label: &'static str, value: String) -> Element {
    rsx! { div { class: "metric", span { "{label}" } strong { "{value}" } } }
}

const GAME_CSS: &str = r#"
:root { --bg:#05070c; --panel:rgba(8,12,20,.92); --fg:#f4f7ff; --dim:#8b98ad; --line:#45627b; --line2:#233444; --accent:#8effa0; --blue:#79c7ff; --mono:'SFMono-Regular',Consolas,monospace; }
html,body { width:100%; height:100%; overflow:hidden !important; overscroll-behavior:none; }
#main { background:transparent; overflow:hidden; max-width:100vw; max-height:100vh; }
.ws-root { pointer-events:none; background:transparent; overflow:hidden; max-width:100vw; max-height:100vh; }
.ws { width:100%; max-width:100%; min-width:0; box-sizing:border-box; }
.panel,.dock { pointer-events:auto; }
.panel { max-width:100%; box-sizing:border-box; }
.panel-body,.game-panel,.combined-panel { min-width:0; max-width:100%; box-sizing:border-box; overflow-wrap:anywhere; }
.dock { background:rgba(5,7,12,.8); }
.ws.maxed .panel { background:transparent; border-color:transparent; box-shadow:none; }
.ws.maxed .panel-head { background:rgba(5,7,12,.88); border:1px solid var(--line2); }
.ws.maxed .panel-body { display:flex; align-items:flex-start; justify-content:center; padding-top:4rem; }
.ws.maxed .game-panel { width:min(360px,calc(100vw - 3rem)); padding:1.25rem; border:1px solid var(--line); background:rgba(8,12,20,.9); box-shadow:0 18px 60px rgba(0,0,0,.45); }
.game-panel { display:flex; flex-direction:column; gap:.8rem; font-family:var(--mono); }
.game-panel h1 { margin:0 0 .25rem; font-size:1.8rem; letter-spacing:.05em; }
.eyebrow { margin:0; color:var(--accent); font-size:.68rem; letter-spacing:.16em; }
.muted { color:var(--dim); line-height:1.45; }
.game-panel button { border:1px solid var(--accent); background:var(--accent); color:#07110a; font:700 .8rem var(--mono); padding:.7rem .9rem; text-transform:uppercase; cursor:pointer; }
.game-panel button.secondary { background:transparent; color:var(--fg); border-color:var(--line); }
.game-panel button.danger { background:transparent; color:#ff9b9b; border-color:#a94b4b; }
.game-panel button:disabled,.game-panel input:disabled { opacity:.42; cursor:not-allowed; }
.game-panel label { color:var(--fg); font-size:.78rem; }
.game-panel input[type=range] { width:100%; accent-color:var(--accent); }
.game-panel .check { display:flex; align-items:center; gap:.5rem; }
.metric-grid { display:grid; grid-template-columns:1fr 1fr; gap:.55rem; }
.metric { border:1px solid var(--line2); padding:.6rem; background:rgba(0,0,0,.22); }
.metric span { display:block; color:var(--dim); font-size:.62rem; letter-spacing:.1em; }
.metric strong { display:block; margin-top:.25rem; color:var(--blue); font-size:1rem; }
.controls { font-size:.68rem; }
.quick-radial-gesture { position:fixed; inset:0; z-index:10000; pointer-events:none; overflow:hidden; font-family:var(--mono); }
.quick-wheel { position:fixed; width:232px; height:232px; transform:translate(-50%,-50%); border-radius:50%; background:radial-gradient(circle,rgba(5,9,16,.97) 0 23%,rgba(18,34,49,.91) 24% 63%,rgba(5,8,14,.1) 64%); filter:drop-shadow(0 14px 35px rgba(0,0,0,.55)); }
.quick-root-wheel { opacity:.82; }
.quick-child-wheel { width:258px; height:258px; }
.quick-tier-label { position:absolute; left:50%; top:12px; width:150px; transform:translateX(-50%); text-align:center; color:var(--blue); font-size:.55rem; letter-spacing:.12em; }
.quick-center { position:absolute; display:grid; place-items:center; left:50%; top:50%; width:54px; height:54px; transform:translate(-50%,-50%); border:1px solid var(--line); border-radius:50%; background:#09111b; color:var(--dim); font:800 .55rem var(--mono); letter-spacing:.08em; }
.quick-item { position:absolute; width:78px; min-height:48px; transform:translate(-50%,-50%); border:1px solid var(--line); border-radius:24px; padding:.35rem .25rem; background:rgba(8,15,25,.96); color:var(--fg); cursor:pointer; text-align:center; box-shadow:0 4px 14px rgba(0,0,0,.4); }
.quick-item.selected { border-color:var(--accent); background:rgba(29,65,70,.98); box-shadow:0 0 0 2px rgba(136,255,207,.18),0 0 22px rgba(136,255,207,.24); }
.quick-item[aria-disabled="true"] { opacity:.28; }
.quick-item b { display:block; color:var(--accent); font:800 .7rem var(--mono); }
.quick-item span { display:block; margin-top:.12rem; color:var(--dim); font:.48rem var(--mono); }
.quick-item.root-slot-0 { left:50%; top:17%; }
.quick-item.root-slot-1 { left:83%; top:50%; }
.quick-item.root-slot-2 { left:50%; top:83%; }
.quick-item.root-slot-3 { left:17%; top:50%; }
.quick-item.slot-0 { left:50%; top:20%; }
.quick-item.slot-1 { left:79%; top:36%; }
.quick-item.slot-2 { left:79%; top:67%; }
.quick-item.slot-3 { left:50%; top:83%; }
.quick-item.slot-4 { left:21%; top:67%; }
.quick-item.slot-5 { left:21%; top:36%; }
.trajectory-legend { pointer-events:none; position:fixed; z-index:9998; left:50%; bottom:15px; transform:translateX(-50%); display:flex; gap:.9rem; padding:.4rem .65rem; border:1px solid rgba(70,98,125,.65); background:rgba(3,7,13,.82); color:var(--dim); font:500 .55rem var(--mono); }
.trajectory-key { display:flex; align-items:center; gap:.35rem; white-space:nowrap; }
.trajectory-key i { display:block; width:22px; height:5px; }
.trajectory-key.coast i { background:repeating-linear-gradient(90deg,#56ccff 0 3px,transparent 3px 7px); }
.trajectory-key.active i { background:repeating-linear-gradient(90deg,#ffb82d 0 5px,transparent 5px 9px); }
.trajectory-key.field i { height:8px; background:linear-gradient(90deg,rgba(121,199,255,.55),rgba(121,199,255,0)); }
.flight-controls { display:flex; flex-direction:column; gap:.55rem; }
.button-row { display:grid; grid-template-columns:1fr 1fr; gap:.5rem; }
.key-reference { color:var(--dim); font-size:.66rem; line-height:1.4; }
.key-reference summary { color:var(--fg); cursor:pointer; }
.key-reference dl { display:grid; grid-template-columns:5rem 1fr; gap:.3rem .55rem; margin:.65rem 0 0; }
.key-reference dt { color:var(--accent); }
.key-reference dd { margin:0; }
.maneuver-planner { overflow:auto; }
.node-pager { display:grid; grid-template-columns:1fr auto 1fr; align-items:center; gap:.45rem; }
.node-pager strong { color:var(--blue); font-size:.72rem; text-align:center; }
.maneuver-gizmo { position:relative; width:238px; height:238px; margin:.15rem auto .35rem; border-radius:50%; background:radial-gradient(circle,rgba(5,11,18,.96) 0 25%,rgba(10,22,32,.78) 26% 58%,transparent 59%); }
.maneuver-orbit-ring { position:absolute; inset:28px; border:1px dashed rgba(125,155,190,.38); border-radius:50%; }
.maneuver-center { position:absolute; left:50%; top:50%; width:72px; height:72px; transform:translate(-50%,-50%); border:1px solid var(--line); border-radius:50%; background:#07101a; display:flex; flex-direction:column; align-items:center; justify-content:center; text-align:center; }
.maneuver-center small { color:var(--dim); font-size:.48rem; } .maneuver-center strong { color:var(--fg); font-size:.8rem; } .maneuver-center span { color:var(--blue); font-size:.46rem; }
.maneuver-handle,.maneuver-time { position:absolute; z-index:2; border:1px solid currentColor !important; background:rgba(6,12,20,.96) !important; color:var(--fg) !important; padding:.35rem !important; cursor:pointer; }
.maneuver-handle { width:72px; min-height:42px; border-radius:22px; }
.maneuver-handle b { display:block; font-size:.7rem; } .maneuver-handle span { display:block; font-size:.46rem; color:var(--dim); }
.maneuver-handle.prograde { right:0; top:50%; transform:translateY(-50%); color:#76ff93 !important; }
.maneuver-handle.retrograde { left:0; top:50%; transform:translateY(-50%); color:#76ff93 !important; }
.maneuver-handle.radial-out { left:50%; top:0; transform:translateX(-50%); color:#6dc8ff !important; }
.maneuver-handle.radial-in { left:50%; bottom:0; transform:translateX(-50%); color:#6dc8ff !important; }
.maneuver-time { width:48px; min-height:30px; border-radius:16px; color:#ffbf69 !important; font-size:.55rem !important; }
.maneuver-time.earlier { left:38px; top:31px; } .maneuver-time.later { right:38px; top:31px; }
.maneuver-fine-ring { display:grid; grid-template-columns:repeat(3,1fr); gap:.35rem; }
.maneuver-precision { border:1px solid var(--line2); padding:.45rem; color:var(--dim); }
.maneuver-precision summary { color:var(--fg); cursor:pointer; margin-bottom:.45rem; }
.combined-panel { height:auto; overflow:visible; display:flex; flex-direction:column; gap:1rem; }
.combined-panel > .game-panel + .game-panel { border-top:1px solid var(--line2); padding-top:1rem; }
.map-panel { height:auto; min-height:0; }
.system-map { width:340px; max-width:100%; height:220px; flex:none; background:rgba(2,5,12,.88); border:1px solid var(--line2); cursor:grab; user-select:none; touch-action:none; }
.system-map.dragging { cursor:grabbing; }
.map-orbit { fill:none; stroke:rgba(105,135,180,.35); stroke-width:1; vector-effect:non-scaling-stroke; }
.map-body { stroke:rgba(255,255,255,.65); stroke-width:1.5; vector-effect:non-scaling-stroke; }
.map-body.primary { fill:#e3bd65; }
.map-body.planet { fill:#78a9c7; }
.map-body.moon { fill:#a8aeba; }
.map-ship { fill:#8effa0; stroke:#07110a; stroke-width:.15; }
.map-legend { margin:0; color:var(--dim); font-size:.6rem; }
.map-footer { display:flex; align-items:center; justify-content:space-between; gap:.5rem; }
.ship-designer { overflow:auto; }
.ship-designer select { width:100%; padding:.45rem; color:var(--fg); background:#080c14; border:1px solid var(--line); font:600 .72rem var(--mono); }
.design-heading { display:flex; align-items:center; justify-content:space-between; gap:.5rem; color:var(--fg); font-size:.78rem; }
.ship-preview-pair { display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1fr); gap:.45rem; }
.projection-preview { min-width:0; position:relative; }
.projection-label { position:absolute; z-index:1; top:.35rem; left:.4rem; color:var(--dim); font-size:.5rem; letter-spacing:.1em; pointer-events:none; }
.ship-preview { display:block; width:100%; aspect-ratio:1.3; min-height:90px; max-height:150px; background:radial-gradient(circle,rgba(80,120,170,.12),rgba(2,5,12,.9)); border:1px solid var(--line2); }
.ship-preview-3d { background:linear-gradient(155deg,rgba(30,48,70,.32),rgba(2,5,12,.95)); }
.preview-axis { stroke:rgba(125,155,190,.14); stroke-width:1; vector-effect:non-scaling-stroke; }
.mass-center { fill:#7dffae; stroke:#06140c; stroke-width:.8; vector-effect:non-scaling-stroke; }
.pressure-center { fill:none; stroke:#ffb36b; stroke-width:1.2; vector-effect:non-scaling-stroke; }
.flight-profile { color:var(--dim); font-size:.64rem; border:1px solid var(--line2); padding:.45rem; }
.flight-profile summary { color:var(--fg); cursor:pointer; }
.profile-legend { display:flex; gap:.8rem; margin:.45rem 0; font-size:.56rem; letter-spacing:.06em; }
.com-key { color:#7dffae; } .cp-key { color:#ffb36b; }
.profile-stats .metric { padding:.4rem; }
.profile-stats .metric strong { font-size:.72rem; }
@media (max-width:700px) { .ship-preview-pair { grid-template-columns:1fr; } }
.rarity { color:var(--dim); text-transform:uppercase; font-size:.62rem; letter-spacing:.1em; }
.rarity-1 { color:#72dc88; } .rarity-2 { color:#65a9ff; } .rarity-3 { color:#c47aff; } .rarity-4 { color:#ffc45e; }
.design-seed { margin:0; color:var(--dim); font-size:.58rem; overflow-wrap:anywhere; }
.parts-manifest { color:var(--dim); font-size:.64rem; }
.parts-manifest summary { color:var(--fg); cursor:pointer; margin-bottom:.35rem; }
.part-row { display:grid; grid-template-columns:4.5rem 1fr auto; align-items:center; gap:.4rem; padding:.24rem 0; border-bottom:1px solid var(--line2); }
.part-row strong { color:var(--fg); font-size:.62rem; }
.part-row em { font-style:normal; font-size:.52rem; }
.timeline-status { color:var(--accent); padding:.45rem; border:1px solid var(--line2); background:rgba(0,0,0,.25); font-size:.7rem; }
.final-score { color:var(--accent); font-size:3rem; font-weight:800; }
.compact { padding:.35rem .5rem !important; font-size:.64rem !important; }
.panel-body:has(.canvas-panel-shell) { padding:0; overflow:hidden; min-width:0; min-height:0; }
.canvas-panel-shell { width:100%; height:100%; min-width:0; min-height:0; display:flex; flex-direction:column; pointer-events:auto; overflow:hidden; }
.canvas-panel-toolbar { display:flex; align-items:center; justify-content:space-between; gap:.6rem; padding:.35rem; color:var(--dim); font:500 .62rem var(--mono); }
.canvas-panel-host { flex:1 1 0; width:100%; height:100%; min-width:0; min-height:0; position:relative; overflow:hidden; background:rgba(0,0,0,.45); border:1px solid var(--line2); box-sizing:border-box; }
.canvas-panel-host:not(.active)::after { content:'GAME VIEW IS RUNNING AS THE FULLSCREEN BACKGROUND'; position:absolute; inset:0; display:grid; place-items:center; padding:1rem; text-align:center; color:var(--dim); font:600 .68rem var(--mono); letter-spacing:.08em; }
.canvas-panel-host #bevy { position:absolute; inset:0; display:block; width:100% !important; height:100% !important; max-width:none; max-height:none; pointer-events:auto; }
/* Panel geometry belongs to panel-kit. The spans in default_layout establish
   the first-run arrangement; leaving tiling as a flex workspace keeps its
   native header reordering and corner-grip resizing available. */
.ws.tiling .panel-system-map .panel-body,.ws.tiling .panel-system-map .map-panel { min-height:0; box-sizing:border-box; }
.ws.tiling .panel-system-map .system-map { width:100%; height:auto; max-height:100%; min-height:0; }
.mobile .ws.tiling .panel-game-view { min-height:32rem; }
.canvas-panel-host,#bevy { overscroll-behavior:none; touch-action:none; }
.planner-mount { min-height:7rem; border:1px dashed var(--line2); background:rgba(0,0,0,.18); }
"#;
