mod game;
mod player;
mod render;
mod world;

use world::generation::World;

use game::*;

use player::PlayerData;

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
    rl.disable_cursor();

    // TODO: we temporarily always create a new GameData on launch
    let data = GameData {
        seed: 42,
        tick_counter: 0,
        player_data: PlayerData::new(),
        world: World::new(),
    };

    let mut game = GameController::new(&mut rl, &thread, data);

    game.run(&mut rl, &thread);
}
