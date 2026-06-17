mod game;
mod player;
mod render;
mod world;
mod settings;

use game::{GameController, GameData};
use player::PlayerData;
use settings::Settings;
use world::generation::World;

const WINDOW_WIDTH: i32 = 1280;
const WINDOW_HEIGHT: i32 = 720;

fn main() {
    let (mut rl, thread) = raylib::init()
        .size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .title("Minecrab")
        .vsync()
        .highdpi()
        .build();

    // Disable exit on esc (default raylib behavior)
    rl.set_exit_key(None);

    // TODO: we temporarily always create a new GameData on launch
    let data = GameData {
        seed: 42,
        tick_counter: 0,
        player_data: PlayerData::new(),
        world: World::new(),
    };

    // FIXME: settings load/store + menu
    let settings = Settings {
        render_distance: 2
    };

    let mut game = GameController::new(&mut rl, &thread, data);

    game.run(&mut rl, &thread, settings);

    game.cleanup();
}
