//! Agent API (and through it MCP) access to the robot showcase commands:
//! `opencad.reach_document`, `opencad.animate_joints_document`, and
//! `opencad.animate_sweep_document`.
//!
//! Each method turns its JSON params into the same flags the CLI parses, so
//! the CLI parser stays the one place that validates units and ranges and the
//! three surfaces cannot drift.  Lengths in params are metres (`_m`), as
//! elsewhere in the Agent API; joint and sweep values are strings with an
//! explicit unit (`"60deg"`, `"32mm"`), as on the command line.

use opencad_ai::{JsonRpcError, JsonRpcRequest, JsonRpcResponse};
use opencad_core::{OpenCadError, Result};
use serde::Deserialize;
use serde_json::Value;

/// Presentation options shared by both animation methods.
#[derive(Debug, Default, Deserialize)]
struct GifParams {
    view: Option<String>,
    style: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    aspect: Option<String>,
    fps: Option<u32>,
    #[serde(default = "yes")]
    caption: bool,
}

fn yes() -> bool {
    true
}

impl GifParams {
    fn flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        let mut push = |flag: &str, value: Option<String>| {
            if let Some(value) = value {
                flags.push(flag.to_string());
                flags.push(value);
            }
        };
        push("--view", self.view.clone());
        push("--style", self.style.clone());
        push("--width", self.width.map(|v| v.to_string()));
        push("--height", self.height.map(|v| v.to_string()));
        push("--aspect", self.aspect.clone());
        push("--fps", self.fps.map(|v| v.to_string()));
        if !self.caption {
            flags.push("--no-caption".into());
        }
        flags
    }
}

#[derive(Debug, Deserialize)]
struct ReachParams {
    path: String,
    /// Instance carrying the tool, `instance:gripper` or `gripper`.
    tool: String,
    /// Tool point in that part's frame, metres.
    #[serde(default)]
    point_m: Option<[f64; 3]>,
    /// World target, metres.
    #[serde(default)]
    target_m: Option<[f64; 3]>,
    #[serde(default)]
    tolerance_m: Option<f64>,
    /// Also write a GIF of the arm moving to its best pose for the target.
    #[serde(default)]
    gif: Option<String>,
    #[serde(flatten)]
    gif_params: GifParams,
    #[serde(default)]
    frames_per_move: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AnimateJointsParams {
    path: String,
    output: String,
    /// Keyframes such as `"shoulder=60deg,elbow=-45deg"`.
    #[serde(default)]
    poses: Vec<String>,
    #[serde(default)]
    frames_per_move: Option<u32>,
    #[serde(flatten)]
    gif_params: GifParams,
}

#[derive(Debug, Deserialize)]
struct AnimateSweepParams {
    path: String,
    output: String,
    param: String,
    /// Start value with a unit, such as `"32mm"`.
    from: String,
    to: String,
    #[serde(default)]
    steps: Option<u32>,
    #[serde(flatten)]
    gif_params: GifParams,
}

fn metres(point: [f64; 3]) -> String {
    format!("{},{},{}m", point[0], point[1], point[2])
}

fn reach(params: ReachParams) -> Result<Value> {
    let mut flags = vec!["--tool".to_string(), params.tool];
    if let Some(point) = params.point_m {
        flags.extend(["--point".into(), metres(point)]);
    }
    if let Some(target) = params.target_m {
        flags.extend(["--target".into(), metres(target)]);
    }
    if let Some(tolerance) = params.tolerance_m {
        flags.extend(["--tolerance".into(), format!("{tolerance}m")]);
    }
    if let Some(gif) = params.gif {
        flags.extend(["--gif".into(), gif]);
        flags.extend(params.gif_params.flags());
        if let Some(frames) = params.frames_per_move {
            flags.extend(["--frames-per-move".into(), frames.to_string()]);
        }
    }
    let options = crate::reach::parse_reach_args(&flags)?;
    Ok(serde_json::to_value(crate::reach::reach(
        &params.path,
        &options,
    )?)?)
}

fn animate_joints(params: AnimateJointsParams) -> Result<Value> {
    let mut flags = params.gif_params.flags();
    for pose in params.poses {
        flags.extend(["--pose".into(), pose]);
    }
    if let Some(frames) = params.frames_per_move {
        flags.extend(["--frames-per-move".into(), frames.to_string()]);
    }
    let options = crate::joint_animation::parse_joint_animation_args(&flags)?;
    Ok(serde_json::to_value(
        crate::joint_animation::animate_joints(&params.path, &params.output, &options)?,
    )?)
}

fn animate_sweep(params: AnimateSweepParams) -> Result<Value> {
    let mut flags = vec![
        "--param".to_string(),
        params.param,
        "--from".into(),
        params.from,
        "--to".into(),
        params.to,
    ];
    if let Some(steps) = params.steps {
        flags.extend(["--steps".into(), steps.to_string()]);
    }
    flags.extend(params.gif_params.flags());
    let options = crate::sweep_animation::parse_sweep_args(&flags)?;
    Ok(serde_json::to_value(
        crate::sweep_animation::animate_sweep(&params.path, &params.output, &options)?,
    )?)
}

/// Presentation keys accepted by every method that can write a GIF.
const GIF_KEYS: [&str; 7] = [
    "view", "style", "width", "height", "aspect", "fps", "caption",
];

fn respond<P: for<'de> Deserialize<'de>>(
    request: &JsonRpcRequest,
    keys: &[&str],
    run: impl FnOnce(P) -> Result<Value>,
) -> JsonRpcResponse {
    // serde cannot deny unknown fields next to `flatten`, so check names here:
    // a misspelt option must fail, not be ignored.
    if let Some(object) = request.params.as_object() {
        if let Some(unknown) = object
            .keys()
            .find(|key| !keys.contains(&key.as_str()) && !GIF_KEYS.contains(&key.as_str()))
        {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(format!("unknown parameter '{unknown}'")),
            );
        }
    }
    let params = match serde_json::from_value::<P>(request.params.clone()) {
        Ok(params) => params,
        Err(error) => {
            return JsonRpcResponse::error(
                request.id.clone(),
                JsonRpcError::invalid_params(error.to_string()),
            )
        }
    };
    match run(params) {
        Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
        Err(OpenCadError::Validation(message)) => {
            JsonRpcResponse::error(request.id.clone(), JsonRpcError::invalid_params(message))
        }
        Err(error) => JsonRpcResponse::error(
            request.id.clone(),
            JsonRpcError::application_error(error.to_string()),
        ),
    }
}

/// Dispatch a robot showcase method, or `None` for any other method.
pub fn handle(request: &JsonRpcRequest) -> Option<JsonRpcResponse> {
    Some(match request.method.as_str() {
        "opencad.reach_document" => respond(
            request,
            &[
                "path",
                "tool",
                "point_m",
                "target_m",
                "tolerance_m",
                "gif",
                "frames_per_move",
            ],
            reach,
        ),
        "opencad.animate_joints_document" => respond(
            request,
            &["path", "output", "poses", "frames_per_move"],
            animate_joints,
        ),
        "opencad.animate_sweep_document" => respond(
            request,
            &["path", "output", "param", "from", "to", "steps"],
            animate_sweep,
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(method: &str, params: Value) -> JsonRpcResponse {
        let request: JsonRpcRequest = serde_json::from_value(json!({
            "jsonrpc": "2.0", "id": 1, "method": method, "params": params
        }))
        .expect("request");
        handle(&request).expect("handled")
    }

    fn example(name: &str) -> String {
        format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn reach_matches_the_cli_and_rejects_bad_params() {
        let response = call(
            "opencad.reach_document",
            json!({
                "path": example("robot_arm_assembly.ocad.d"),
                "tool": "gripper",
                "point_m": [0.0, 0.04, 0.007],
                "target_m": [0.0, 0.33, 0.063]
            }),
        );
        let result = response.result.expect("result");
        assert_eq!(result["target"]["reachable"], json!(false));
        let gap = result["target"]["gap_mm"].as_f64().expect("gap");
        assert!((gap - 20.0).abs() < 0.1, "{gap}");

        let unknown = call(
            "opencad.reach_document",
            json!({ "path": example("robot_arm_assembly.ocad.d"), "tool": "gripper", "speed": 2 }),
        );
        assert_eq!(unknown.error.expect("error").code, -32602);
        let negative = call(
            "opencad.reach_document",
            json!({ "path": example("robot_arm_assembly.ocad.d"), "tool": "gripper", "tolerance_m": -1.0 }),
        );
        assert_eq!(negative.error.expect("error").code, -32602);
        let other = serde_json::from_value::<JsonRpcRequest>(
            json!({ "jsonrpc": "2.0", "id": 1, "method": "opencad.inspect", "params": {} }),
        )
        .expect("request");
        assert!(handle(&other).is_none());
    }

    #[test]
    fn animations_write_gifs_through_the_cli_parsers() {
        let dir = std::env::temp_dir();
        let joints = dir.join(format!("musubicad-api-joints-{}.gif", std::process::id()));
        let response = call(
            "opencad.animate_joints_document",
            json!({
                "path": example("robot_arm_assembly.ocad.d"),
                "output": joints,
                "poses": ["shoulder=30deg,elbow=-20deg"],
                "frames_per_move": 2,
                "width": 96,
                "aspect": "1:1",
                "style": "dark"
            }),
        );
        let result = response.result.expect("joints");
        assert_eq!(result["frames"], json!(4));
        assert_eq!(result["height_px"], json!(96));
        assert!(std::fs::read(&joints).expect("gif").starts_with(b"GIF"));
        let _ = std::fs::remove_file(&joints);

        let past_limit = call(
            "opencad.animate_joints_document",
            json!({
                "path": example("robot_arm_assembly.ocad.d"),
                "output": joints,
                "poses": ["elbow=150deg"]
            }),
        );
        assert_eq!(past_limit.error.expect("error").code, -32602);

        let sweep = dir.join(format!("musubicad-api-sweep-{}.gif", std::process::id()));
        let response = call(
            "opencad.animate_sweep_document",
            json!({
                "path": example("bearing_carrier.ocad.d"),
                "output": sweep,
                "param": "boss_height",
                "from": "14mm",
                "to": "20mm",
                "steps": 2,
                "width": 64,
                "height": 48,
                "caption": false
            }),
        );
        let result = response.result.expect("sweep");
        assert_eq!(result["samples"].as_array().expect("samples").len(), 2);
        let _ = std::fs::remove_file(&sweep);
    }
}
