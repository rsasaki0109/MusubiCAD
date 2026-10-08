mod agent;
mod animate;
mod commands;
mod diff;
mod export;
mod git_driver;
mod git_workflow;
mod import;
mod joint_animation;
mod mcp;
mod mesh;
mod mesh_formats;
mod new;
mod patch;
mod pick;
mod plugin;
mod policy_check;
mod preview;
mod regen;
mod review;
mod review_gif;
mod scene_query;
mod topo_sync;
mod urdf;
mod view;

fn main() {
    if let Err(err) = commands::run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
