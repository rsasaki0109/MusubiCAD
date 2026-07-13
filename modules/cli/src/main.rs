mod agent;
mod animate;
mod commands;
mod diff;
mod export;
mod mesh;
mod new;
mod patch;
mod pick;
mod regen;
mod scene_query;
mod topo_sync;
mod view;

fn main() {
    if let Err(err) = commands::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
