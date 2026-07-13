use crate::GameState;
use bevy::prelude::*;

pub struct MenuPlugin;

/// This plugin is responsible for the game menu
/// The menu is only drawn during the State `GameState::Menu` and is removed when that state is exited
impl Plugin for MenuPlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(target_arch = "wasm32")]
        return;

        #[cfg(not(target_arch = "wasm32"))]
        _app.add_systems(OnEnter(GameState::Menu), setup_menu)
            .add_systems(Update, click_menu_button.run_if(in_state(GameState::Menu)))
            .add_systems(OnExit(GameState::Menu), cleanup_menu);
    }
}

#[derive(Component, Clone)]
struct ButtonColors {
    normal: Color,
    hovered: Color,
}

impl Default for ButtonColors {
    fn default() -> Self {
        ButtonColors {
            normal: Color::BLACK,
            hovered: Color::linear_rgb(0.2, 0.2, 0.2),
        }
    }
}

#[derive(Component)]
struct Menu;

#[derive(Component)]
struct MenuCamera;

#[derive(Component)]
struct ChangeState(GameState);

fn setup_menu(mut commands: Commands) {
    info!("Setting up main menu");
    commands.spawn((Camera2d, Msaa::Off, MenuCamera));

    // Main menu container
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Menu,
        ))
        .with_children(|children| {
            // Game title
            children.spawn((
                Text::new("HOPPER"),
                TextFont {
                    font_size: 80.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(20.0)),
                    ..default()
                },
            ));

            // Subtitle
            children.spawn((
                Text::new("Lunar Lander"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::linear_rgb(0.7, 0.7, 0.7)),
                Node {
                    margin: UiRect::bottom(Val::Px(80.0)),
                    ..default()
                },
            ));

            // Play button
            let button_colors = ButtonColors::default();
            children
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(60.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        margin: UiRect::bottom(Val::Px(20.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..Default::default()
                    },
                    BackgroundColor(button_colors.normal),
                    BorderColor(Color::WHITE),
                    button_colors.clone(),
                    ChangeState(GameState::Playing),
                ))
                .with_child((
                    Text::new("PLAY"),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));

            // Settings button
            children
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(200.0),
                        height: Val::Px(60.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border: UiRect::all(Val::Px(2.0)),
                        ..Default::default()
                    },
                    BackgroundColor(button_colors.normal),
                    BorderColor(Color::WHITE),
                    button_colors,
                    ChangeState(GameState::Settings),
                ))
                .with_child((
                    Text::new("SETTINGS"),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
        });
}

fn click_menu_button(
    mut next_state: ResMut<NextState<GameState>>,
    mut interaction_query: Query<
        (
            &Interaction,
            &mut BackgroundColor,
            &ButtonColors,
            Option<&ChangeState>,
        ),
        (Changed<Interaction>, With<Button>),
    >,
) {
    for (interaction, mut color, button_colors, change_state) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                if let Some(state) = change_state {
                    next_state.set(state.0.clone());
                }
            }
            Interaction::Hovered => {
                *color = button_colors.hovered.into();
            }
            Interaction::None => {
                *color = button_colors.normal.into();
            }
        }
    }
}

fn cleanup_menu(
    mut commands: Commands,
    menu: Query<Entity, With<Menu>>,
    cameras: Query<Entity, With<MenuCamera>>,
) {
    for entity in menu.iter() {
        commands.entity(entity).despawn();
    }
    for entity in cameras.iter() {
        commands.entity(entity).despawn();
    }
}
