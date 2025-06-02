use crate::game::GameData;
use crate::GameState;
use bevy::prelude::*;

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::GameOver), setup_game_over)
            .add_systems(Update, handle_game_over_input.run_if(in_state(GameState::GameOver)))
            .add_systems(OnExit(GameState::GameOver), cleanup_game_over);
    }
}

#[derive(Component)]
struct GameOver;

#[derive(Component)]
struct GameOverCamera;

#[derive(Component)]
struct RestartButton;

#[derive(Component)]
struct MenuButton;

fn setup_game_over(mut commands: Commands, game_data: Res<GameData>) {
    info!("Setting up game over screen");
    commands.spawn((Camera2d, Msaa::Off, GameOverCamera));

    // Game over screen container
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
            GameOver,
        ))
        .with_children(|children| {
            // Game Over title
            children.spawn((
                Text::new("GAME OVER"),
                TextFont {
                    font_size: 80.0,
                    ..default()
                },
                TextColor(Color::WHITE),
                Node {
                    margin: UiRect::bottom(Val::Px(30.0)),
                    ..default()
                },
            ));

            // Score display
            children.spawn((
                Text::new(format!("Time Survived: {:.1}s", game_data.time)),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::linear_rgb(0.8, 0.8, 0.8)),
                Node {
                    margin: UiRect::bottom(Val::Px(20.0)),
                    ..default()
                },
            ));

            children.spawn((
                Text::new(format!("Final Score: {}", game_data.score)),
                TextFont {
                    font_size: 32.0,
                    ..default()
                },
                TextColor(Color::linear_rgb(0.8, 0.8, 0.8)),
                Node {
                    margin: UiRect::bottom(Val::Px(60.0)),
                    ..default()
                },
            ));

            // Restart button
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
                    RestartButton,
                ))
                .with_child((
                    Text::new("RESTART"),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));

            // Menu button
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
                    MenuButton,
                ))
                .with_child((
                    Text::new("MAIN MENU"),
                    TextFont {
                        font_size: 32.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
        });
}

fn handle_game_over_input(
    mut next_state: ResMut<NextState<GameState>>,
    mut interaction_query: Query<
        (&Interaction, Option<&RestartButton>, Option<&MenuButton>),
        (Changed<Interaction>, With<Button>),
    >,
    keyboard_input: Res<ButtonInput<KeyCode>>,
) {
    // Handle button clicks
    for (interaction, restart, menu) in interaction_query.iter_mut() {
        if *interaction == Interaction::Pressed {
            if restart.is_some() {
                next_state.set(GameState::Playing);
                return;
            } else if menu.is_some() {
                next_state.set(GameState::Menu);
                return;
            }
        }
    }

    // Handle keyboard shortcuts
    if keyboard_input.just_pressed(KeyCode::Space) {
        next_state.set(GameState::Playing);
    } else if keyboard_input.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Menu);
    }
}

fn cleanup_game_over(mut commands: Commands, game_over: Query<Entity, With<GameOver>>, cameras: Query<Entity, With<GameOverCamera>>) {
    for entity in game_over.iter() {
        commands.entity(entity).despawn();
    }
    for entity in cameras.iter() {
        commands.entity(entity).despawn();
    }
}
