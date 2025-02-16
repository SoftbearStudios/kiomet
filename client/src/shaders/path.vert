// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

attribute vec2 position;
attribute vec4 transform;
attribute vec4 color;
uniform mat3 uView;
varying vec4 vColor;

void main() {
    // Scale, rotate, then translate.
    float s = sin(transform.z);
    float c = cos(transform.z);
    float r = transform.z < 0.0 ? -1.0: 1.0;
    mat2 matrix = mat2(c, s, -s, c);
    vec2 vPosition = matrix * (position * vec2(r, 1.0) * transform.w) + transform.xy;
    gl_Position = vec4(uView * vec3(vPosition, 1.0), 1.0);
    vColor = color;

    // Premultiply alpha.
    vColor.rgb *= color.a;
}
