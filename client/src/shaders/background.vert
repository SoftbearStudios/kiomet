// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

attribute vec4 position;
attribute vec2 uv;
uniform mat3 uCamera;
uniform mat3 uTexture;
varying vec2 vCell;

void main() {
    gl_Position = position;
    vCell = (uCamera * vec3(uv, 1.0)).xy * (1.0 / 5.0);
}
