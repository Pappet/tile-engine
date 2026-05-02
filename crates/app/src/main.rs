use bevy::prelude::*;

fn main() {
    let mut app = App::new();
    
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Tile Engine".to_string(),
            ..default()
        }),
        ..default()
    }));

    #[cfg(feature = "profile")]
    {
        puffin::set_scopes_on(true);
        let server_addr = format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT);
        let server = puffin_http::Server::new(&server_addr).unwrap();
        eprintln!("Puffin profiler running on {}", server_addr);
        app.insert_non_send_resource(server);
        app.add_systems(Update, profile_system);
    }

    app.add_systems(Update, dummy_system);

    app.run();
}

#[cfg(feature = "profile")]
fn profile_system() {
    puffin::GlobalProfiler::lock().new_frame();
}

fn dummy_system() {
    #[cfg(feature = "profile")]
    puffin::profile_function!();
    // Do nothing
}
