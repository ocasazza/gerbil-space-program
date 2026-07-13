// disable console on windows for release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};
use bevy::winit::WinitWindows;
use bevy::DefaultPlugins;
#[cfg(target_arch = "wasm32")]
use gerbil_space_program::launch_web_ui;
use gerbil_space_program::GamePlugin;
use std::io::Cursor;
use winit::window::Icon;

fn main() {
    #[cfg(target_arch = "wasm32")]
    launch_web_ui();

    App::new()
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Gerbil Space Program".to_string(),
                        // Bind to canvas included in `index.html`
                        canvas: Some("#bevy".to_owned()),
                        fit_canvas_to_parent: true,
                        // Browser canvas already has CSS scaling. Rendering at
                        // the host Mac's Retina factor doubled both axes even
                        // when Chrome reported devicePixelRatio=1, quadrupling
                        // fragment work inside the panel. Keep one backing
                        // pixel per panel pixel on web; native retains its OS
                        // scale factor and full-resolution window rendering.
                        resolution: web_window_resolution(),
                        // Tells wasm not to override default event handling, like F5 and Ctrl+R
                        prevent_default_event_handling: false,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                }),
        )
        .add_plugins(GamePlugin)
        .add_systems(Startup, set_window_icon)
        .run();
}

fn web_window_resolution() -> WindowResolution {
    #[cfg(target_arch = "wasm32")]
    {
        return WindowResolution::default().with_scale_factor_override(1.0);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        WindowResolution::default()
    }
}

// Sets the icon on windows and X11
fn set_window_icon(
    windows: NonSend<WinitWindows>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
) -> Result {
    let primary_entity = primary_window.single()?;
    let Some(primary) = windows.get_window(primary_entity) else {
        return Err(BevyError::from("No primary window!"));
    };
    let icon_buf = Cursor::new(include_bytes!(
        "../build/macos/AppIcon.iconset/icon_256x256.png"
    ));
    if let Ok(image) = image::load(icon_buf, image::ImageFormat::Png) {
        let image = image.into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        let icon = Icon::from_rgba(rgba, width, height).unwrap();
        primary.set_window_icon(Some(icon));
    };

    Ok(())
}
