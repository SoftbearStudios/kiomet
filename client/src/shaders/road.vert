// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

attribute vec2 position;
attribute vec2 center;
attribute vec2 scale;
attribute float rotation;
attribute vec4 color;
attribute float end_alpha;
attribute vec2 uv;
uniform mat3 uView;
uniform float uTime;
varying vec4 vColor;
varying vec3 vCircle;
varying float vUv;

void main() {
    float a = position.x < 0.0 ? color.a : end_alpha;
    vColor = vec4(color.rgb, a);

    float length = scale.x + scale.y;
    float o = scale.x / length;
    vCircle = vec3(position * 2.0, o) * vec2(1.0 / (1.0 - o), 1.0).xyx;
    vUv = float(uv.y > 0.001) * (mix(uv.x, uv.y, position.x / o + 0.5) - uTime * 0.5);

    vec2 vPosition = position * vec2(length, scale.y);
    float s = sin(rotation);
    float c = cos(rotation);
    mat2 matrix = mat2(c, s, -s, c);
    vPosition = matrix * vPosition + center;

    gl_Position = vec4(uView * vec3(vPosition, 1.0), 1.0);
}
