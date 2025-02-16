// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

precision mediump float;
varying vec4 vColor;
varying vec3 vCircle;
varying highp float vUv;

void main() {
    vec4 color = vColor;
    vec2 d = vec2(max(abs(vCircle.x) - vCircle.z, 0.0), vCircle.y);
    color.a *= smoothstep(-1.0, -0.33, -dot(d, d));
    float t = mod(vUv + 0.05, 0.75);
    color.a *= smoothstep(-0.27, -0.2, -(t + abs(d.y) * 0.15)) * smoothstep(0.0, 0.035, t) * 0.75 + 0.25;

    // Premultiply alpha.
    color.rgb *= color.a;
    gl_FragColor = color;
}
