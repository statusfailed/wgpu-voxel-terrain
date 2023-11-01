mod state;
mod run;
mod camera;
mod texture;
mod voxel;
mod compute;
mod constants;

use run::run;

fn main() {
    pollster::block_on(run());
    println!("Hello, world!");
}
