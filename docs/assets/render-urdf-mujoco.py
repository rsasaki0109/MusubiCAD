"""Render an exported MusubiCAD URDF in MuJoCo with its joints swept.

    musubicad export examples/robot_arm_assembly.ocad.d /tmp/arm/robot_arm.urdf
    MUJOCO_GL=osmesa python docs/assets/render-urdf-mujoco.py \
        /tmp/arm/robot_arm.urdf docs/assets/urdf-mujoco.gif

Needs `pip install mujoco pillow` (MuJoCo 3.15 was used) and an OpenGL
backend (OSMesa or EGL). The scene adds only a floor, a light, a camera, and
per-link colours; links, joints, meshes, and inertials come unchanged from
the URDF.
"""
import math, os, re, sys
import mujoco, numpy as np
from PIL import Image

urdf, out_gif = sys.argv[1], sys.argv[2]
model = mujoco.MjModel.from_xml_path(urdf)
scratch = os.path.join(os.path.dirname(os.path.abspath(out_gif)), "_scene.xml")
mujoco.mj_saveLastXML(scratch, model)
xml = open(scratch).read()
os.remove(scratch)
xml = xml.replace("<compiler ", f'<compiler meshdir="{os.path.dirname(os.path.abspath(urdf))}" ', 1)

def look_at(eye, target):
    eye, target = np.array(eye), np.array(target)
    forward = target - eye; forward /= np.linalg.norm(forward)
    right = np.cross(forward, [0, 0, 1]); right /= np.linalg.norm(right)
    up = np.cross(right, forward)
    return " ".join(f"{v:.4f}" for v in [*right, *up])

eye, target = [0.36, -0.20, 0.30], [0.05, 0.11, 0.03]
xml = xml.replace("<worldbody>", f"""<asset>
    <texture name="sky" type="skybox" builtin="gradient" rgb1="0.97 0.98 1" rgb2="0.80 0.85 0.92" width="256" height="256"/>
    <texture name="grid" type="2d" builtin="checker" rgb1="0.92 0.93 0.95" rgb2="0.84 0.86 0.90" width="256" height="256"/>
    <material name="grid" texture="grid" texrepeat="12 12" reflectance="0.08"/>
  </asset>
  <visual><headlight ambient="0.45 0.45 0.45" diffuse="0.45 0.45 0.45" specular="0.1 0.1 0.1"/><quality shadowsize="4096"/><global offwidth="960" offheight="720"/></visual>
  <worldbody>
    <light pos="0.4 -0.4 1.0" dir="-0.35 0.35 -1" directional="true" castshadow="true" diffuse="0.6 0.6 0.6"/>
    <geom name="floor" type="plane" size="0.6 0.6 0.01" material="grid"/>
    <camera name="hero" pos="{' '.join(map(str, eye))}" xyaxes="{look_at(eye, target)}"/>""", 1)
colors = iter(["0.30 0.33 0.38 1", "0.13 0.47 0.84 1", "0.95 0.55 0.15 1", "0.20 0.70 0.45 1"])
xml = re.sub(r'type="mesh"', lambda m: f'type="mesh" rgba="{next(colors, "0.6 0.6 0.6 1")}"', xml)
model = mujoco.MjModel.from_xml_string(xml)
data = mujoco.MjData(model)
renderer = mujoco.Renderer(model, 480, 640)
frames = []
for i in range(90):
    t = 2 * math.pi * i / 90
    data.qpos[:] = [0.9 * math.sin(t), 0.8 * math.sin(2 * t), 1.5 * math.sin(t + 1)]
    mujoco.mj_forward(model, data)
    renderer.update_scene(data, camera="hero")
    frames.append(Image.fromarray(renderer.render()))
frames[0].save(out_gif, save_all=True, append_images=frames[1:], duration=50, loop=0, optimize=True)
frames[22].save(out_gif.replace(".gif", ".png"))
print("frames", len(frames), "size", os.path.getsize(out_gif))
