// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

precision mediump float;

varying highp vec2 vCell;

uniform sampler2D uTowers;
uniform float uDerivative;
uniform highp vec4 uTransform;
uniform highp vec2 uUnit;

/* Modified source from https://www.shadertoy.com/view/4dS3Wd ----> */
// By Morgan McGuire @morgan3d, http://graphicscodex.com
// Reuse permitted under the BSD license.

// Precision-adjusted variations of https://www.shadertoy.com/view/4djSRW
float hash(vec2 p) {vec3 p3 = fract(vec3(p.xyx) * 0.13); p3 += dot(p3, p3.yzx + 3.333); return fract((p3.x + p3.y) * p3.z); }

float noise(highp vec2 x) {
    highp vec2 i = floor(x);
    vec2 f = x - i;
    float a = hash(i);
    float b = hash(i + vec2(1.0, 0.0));
    float c = hash(i + vec2(0.0, 1.0));
    float d = hash(i + vec2(1.0, 1.0));
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(a, b, u.x) + (c - a) * u.y * (1.0 - u.x) + (d - b) * u.x * u.y;
}
// End of modified source.

float quantize(float v, float steps, float border) {
    float x = v * steps;
    float a = floor(x);
    float b = ceil(x);
    float c = smoothstep(b - border, b, x);
    return mix(a, b, c) * (1.0 / steps);
}

int modI(int a, int b) {
    return a - (a / b) * b;
}

vec3 shieldColor(vec3 color, float id, float f) {
    bool is_some = id != 0.0;
    bool is_self = id < (1.5 / 255.0);
    bool is_ally = id < (127.5 / 255.0);
    vec3 c = (is_self ? vec3(0.45, 0.73, 1.0) : is_ally ? vec3(0.53, 0.27, 0.99) : vec3(0.75, 0.22, 0.17)) * float(is_some);
    float b = smoothstep(-0.14, 0.02, -f) * float(is_some);
    return mix(color + c * 0.14, c * 0.85 + 0.15, b);
}

void main() {
    vec3 midnight = vec3(0.173, 0.243, 0.314);
    float falloff = smoothstep(-0.2, -0.05, -uDerivative);
    vec3 color = midnight * 0.8 + quantize(noise(vCell), 5.0, 0.05) * 0.04 * falloff;

    highp vec2 towerID = floor(vCell);
    vec2 cellFract = vCell - towerID;
    highp vec2 towerUV = towerID * uTransform.xy + uTransform.zw;

    float da[9];
    float ida[9];
    float num;
    float denom;

    for (int index = 0; index < 9; index++) {
        vec2 offset = vec2(index / 3 - 1, modI(index, 3) - 1);
        highp vec2 uv = towerUV + uUnit * offset;
        vec4 v = texture2D(uTowers, uv);

        vec2 delta = offset + v.xy - cellFract;
        float d = dot(delta, delta);

        float weight = max(1.0 - d, 0.004);
        weight *= weight;
        num += v.w * weight;
        denom += weight;

        da[index] = d;
        ida[index] = v.z;
    }

    float x;
    #define C(a, b) if (da[a] > da[b]) { x = da[a]; da[a] = da[b]; da[b] = x; x = ida[a]; ida[a] = ida[b]; ida[b] = x; }

    // Use sorting network with 25 CAS.
    C(0,1)C(3,4)C(6,7)
    C(1,2)C(4,5)C(7,8)
    C(0,1)C(3,4)C(6,7)C(2,5)
    C(0,3)C(1,4)C(5,8)
    C(3,6)C(4,7)C(2,5)
    C(0,3)C(1,4)C(5,7)C(2,6)
    C(1,3)C(4,6)
    C(2,4)C(5,6)
    C(2,3)

    float visibility = num / denom;

    float e = 0.0;
    float f = 1.0;
    float sqrt0 = sqrt(da[0]);

    // Use the 5 closest to suport up to the elusive equidistant pentagon.
    for (int i = 1; i < 5; i++) {
        e = (1.0 - e) * float(abs(ida[0] - ida[i]) > (0.5 / 255.0));
        float v = mix(1.0, sqrt(da[i]) - sqrt0, e);
        f = min(v, f);
    }

    vec3 shield1 = shieldColor(color, ida[0], f);
    vec3 shield2 = shieldColor(color, ida[1], f);

    float df = uDerivative * 0.3;
    vec3 shield = mix(shield2, shield1, smoothstep(-df, df, f));

    color = mix(color * 0.8, shield, visibility);
    gl_FragColor = vec4(color, 1.0);
}
