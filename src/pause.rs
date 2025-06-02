use crate::GameState;
use bevy::prelude::*;

pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Paused), setup_pause)
            .add_systems(Update, handle_pause_input.run_if(in_state(GameState::Paused)))
            .add_systems(OnExit(GameState::Paused), cleanup_pause);
    }
}

#[derive(Component)]
struct PauseMenu;

#[derive(Component)]
struct PauseCamera;

#[derive(Component)]
struct ResumeButton;

#[derive(Component)]
struct PauseMenuButton;

fn setup_pause(mut commands: Commands) {
    info!("Setting up pause menu");
    commands.spawn((Camera2d, Msaa::Off, PauseCamera));

    // Pause menu container
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
            PauseMenu,
        ))
        .with_children(|children| {
            // Pause title
            children.spawn((
                Text::new("PAUSED"),
                TextFont {
                    font_size: 80.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(50.0)),
                    ..default()
                },
            ));

            // Resume button
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
                    BackgroundColor(Color::BLACK),
                    BorderColor(Color::WHITE),
                    ResumeButton,
                ))
                .with_child((
                    Text::new("RESUME"),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));

            // Main menu button
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
                    BackgroundColor(Color::BLACK),
                    BorderColor(Color::WHITE),
                    PauseMenuButton,
                ))
                .with_child((
                    Text::new("MAIN MENU"),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));

            // Instructions
            children.spawn((
                Text::new("Press ESC to resume"),
                TextFont {
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::linear_rgb(0.7, 0.7, 0.7)),
                Node {
                    margin: UiRect::top(Val::Px(40.0)),
                    ..default()
                },
            ));
        });
}

fn handle_pause_input(
    mut next_state: ResMut<NextState<GameState>>,
    mut interaction_query: Query<
        (&Interaction, Option<&ResumeButton>, Option<&PauseMenuButton>),
        (Changed<Interaction>, With<Button>),
    >,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    // Handle button clicks
    for (interaction, resume, menu) in interaction_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            if resume.is_some() {
                next_state.set(GameState::Playing);
                return;
            } else if menu.is_some() {
                next_state.set(GameState::Menu);
                return;
            }
        }
    }

    // Handle keyboard shortcuts
    if keyboard_input.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Playing);
    }
}

fn cleanup_pause(mut commands: Commands, pause_menu: Query<Entity, With<PauseMenu>>, cameras: Query<Entity, With<PauseCamera>>) {
    for entity in pause_menu.iter() {
        commands.entity(entity).despawn();
    }
    for entity in cameras.iter() {
        commands.entity(entity).despawn();
    }
}
