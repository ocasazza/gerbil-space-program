use crate::GameState;
use bevy::prelude::*;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, _app: &mut App) {
        #[cfg(target_arch = "wasm32")]
        return;

        #[cfg(not(target_arch = "wasm32"))]
        _app.add_systems(OnEnter(GameState::Settings), setup_settings)
            .add_systems(
                Update,
                handle_settings_input.run_if(in_state(GameState::Settings)),
            )
            .add_systems(OnExit(GameState::Settings), cleanup_settings);
    }
}

#[derive(Component)]
struct Settings;

#[derive(Component)]
struct SettingsCamera;

#[derive(Component)]
struct BackButton;

fn setup_settings(mut commands: Commands) {
    info!("Setting up settings menu");
    commands.spawn((Camera2d, Msaa::Off, SettingsCamera));

    // Settings menu container
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
            Settings,
        ))
        .with_children(|children| {
            // Settings title
            children.spawn((
                Text::new("SETTINGS"),
                TextFont {
                    font_size: 60.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(50.0)),
                    ..default()
                },
            ));

            // Placeholder settings text
            children.spawn((
                Text::new("Settings will be implemented here"),
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

            // Back button
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
                    BackButton,
                ))
                .with_child((
                    Text::new("BACK"),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
        });
}

fn handle_settings_input(
    mut next_state: ResMut<NextState<GameState>>,
    mut interaction_query: Query<&Interaction, (Changed<Interaction>, With<BackButton>)>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    // Handle back button click
    for interaction in interaction_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::Menu);
            return;
        }
    }

    // Handle escape key
    if keyboard_input.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Menu);
    }
}

fn cleanup_settings(
    mut commands: Commands,
    settings: Query<Entity, With<Settings>>,
    cameras: Query<Entity, With<SettingsCamera>>,
) {
    for entity in settings.iter() {
        commands.entity(entity).despawn();
    }
    for entity in cameras.iter() {
        commands.entity(entity).despawn();
    }
}
