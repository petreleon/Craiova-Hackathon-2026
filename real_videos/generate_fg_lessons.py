#!/usr/bin/env python3
"""
Fast Synthetic 3D FlightGear Cockpit Video Generator for Lessons 1-6 (AERO-SHIELD LRCV RWY 09).
Generates fg_lesson_1.mp4 .. fg_lesson_6.mp4 (800x360, 30 FPS, 8.0s = 240 frames, libx264, yuv420p)
using an on-the-fly compiled C renderer piped into ffmpeg.
"""
import os
import subprocess
import sys
import time

OUT_DIR = "/home/petreleon/code/Craiova Hackathon 2026/real_videos"
os.makedirs(OUT_DIR, exist_ok=True)

C_SOURCE = r"""
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>

#define W 800
#define H 360
#define FPS 30
#define FRAMES 240

static uint8_t fb[H][W][3];

static inline void clamp_put(int x, int y, int r, int g, int b) {
    if ((unsigned)x >= W || (unsigned)y >= H) return;
    fb[y][x][0] = (uint8_t)(r < 0 ? 0 : (r > 255 ? 255 : r));
    fb[y][x][1] = (uint8_t)(g < 0 ? 0 : (g > 255 ? 255 : g));
    fb[y][x][2] = (uint8_t)(b < 0 ? 0 : (b > 255 ? 255 : b));
}

static inline void blend_put(int x, int y, int r, int g, int b, float alpha) {
    if ((unsigned)x >= W || (unsigned)y >= H) return;
    int or_ = fb[y][x][0];
    int og_ = fb[y][x][1];
    int ob_ = fb[y][x][2];
    fb[y][x][0] = (uint8_t)(or_ * (1.0f - alpha) + r * alpha);
    fb[y][x][1] = (uint8_t)(og_ * (1.0f - alpha) + g * alpha);
    fb[y][x][2] = (uint8_t)(ob_ * (1.0f - alpha) + b * alpha);
}

static void fill_rect(int x0, int y0, int w, int h, int r, int g, int b, float alpha) {
    for (int y = y0; y < y0 + h; y++) {
        if ((unsigned)y >= H) continue;
        for (int x = x0; x < x0 + w; x++) {
            if ((unsigned)x >= W) continue;
            if (alpha >= 0.99f) clamp_put(x, y, r, g, b);
            else blend_put(x, y, r, g, b, alpha);
        }
    }
}

static void draw_line(float x0, float y0, float x1, float y1, int thick, int r, int g, int b, float alpha) {
    float dx = x1 - x0, dy = y1 - y0;
    float steps = fabsf(dx) > fabsf(dy) ? fabsf(dx) : fabsf(dy);
    if (steps < 1.0f) steps = 1.0f;
    float sx = dx / steps, sy = dy / steps;
    float x = x0, y = y0;
    int rad = thick / 2;
    for (int i = 0; i <= (int)steps; i++) {
        for (int ty = -rad; ty <= rad; ty++) {
            for (int tx = -rad; tx <= rad; tx++) {
                if (alpha >= 0.99f) clamp_put((int)x + tx, (int)y + ty, r, g, b);
                else blend_put((int)x + tx, (int)y + ty, r, g, b, alpha);
            }
        }
        x += sx; y += sy;
    }
}

static void draw_arrow(float x0, float y0, float x1, float y1, int thick, int r, int g, int b) {
    draw_line(x0, y0, x1, y1, thick, r, g, b, 1.0f);
    float ang = atan2f(y1 - y0, x1 - x0);
    float head = 8.0f + thick;
    draw_line(x1, y1, x1 - head * cosf(ang - 0.5f), y1 - head * sinf(ang - 0.5f), thick, r, g, b, 1.0f);
    draw_line(x1, y1, x1 - head * cosf(ang + 0.5f), y1 - head * sinf(ang + 0.5f), thick, r, g, b, 1.0f);
}

static void draw_circle(int cx, int cy, int rad, int fill, int r, int g, int b, float alpha) {
    int r2 = rad * rad;
    int r_in2 = (rad - 2) * (rad - 2);
    for (int dy = -rad; dy <= rad; dy++) {
        for (int dx = -rad; dx <= rad; dx++) {
            int d2 = dx * dx + dy * dy;
            if (d2 <= r2 && (fill || d2 >= r_in2)) {
                if (alpha >= 0.99f) clamp_put(cx + dx, cy + dy, r, g, b);
                else blend_put(cx + dx, cy + dy, r, g, b, alpha);
            }
        }
    }
}

/* 5x7 ASCII bitmap font for uppercase, digits, symbols */
static const uint8_t font5x7[96][7] = {
    [' ' - 32] = {0x00,0x00,0x00,0x00,0x00,0x00,0x00},
    ['!' - 32] = {0x04,0x04,0x04,0x04,0x04,0x00,0x04},
    ['"' - 32] = {0x0A,0x0A,0x00,0x00,0x00,0x00,0x00},
    ['#' - 32] = {0x0A,0x1F,0x0A,0x0A,0x1F,0x0A,0x00},
    ['$' - 32] = {0x04,0x0F,0x14,0x0E,0x05,0x1E,0x04},
    ['%' - 32] = {0x18,0x19,0x02,0x04,0x08,0x13,0x03},
    ['&' - 32] = {0x0C,0x12,0x14,0x08,0x15,0x12,0x0D},
    ['\''- 32] = {0x04,0x04,0x00,0x00,0x00,0x00,0x00},
    ['(' - 32] = {0x02,0x04,0x08,0x08,0x08,0x04,0x02},
    [')' - 32] = {0x08,0x04,0x02,0x02,0x02,0x04,0x08},
    ['*' - 32] = {0x00,0x04,0x15,0x0E,0x15,0x04,0x00},
    ['+' - 32] = {0x00,0x04,0x04,0x1F,0x04,0x04,0x00},
    [',' - 32] = {0x00,0x00,0x00,0x00,0x04,0x04,0x08},
    ['-' - 32] = {0x00,0x00,0x00,0x1F,0x00,0x00,0x00},
    ['.' - 32] = {0x00,0x00,0x00,0x00,0x00,0x0C,0x0C},
    ['/' - 32] = {0x01,0x02,0x02,0x04,0x08,0x08,0x10},
    ['0' - 32] = {0x0E,0x11,0x13,0x15,0x19,0x11,0x0E},
    ['1' - 32] = {0x04,0x0C,0x04,0x04,0x04,0x04,0x0E},
    ['2' - 32] = {0x0E,0x11,0x01,0x06,0x08,0x10,0x1F},
    ['3' - 32] = {0x1F,0x02,0x04,0x06,0x01,0x11,0x0E},
    ['4' - 32] = {0x02,0x06,0x0A,0x12,0x1F,0x02,0x02},
    ['5' - 32] = {0x1F,0x10,0x1E,0x01,0x01,0x11,0x0E},
    ['6' - 32] = {0x06,0x08,0x10,0x1E,0x11,0x11,0x0E},
    ['7' - 32] = {0x1F,0x01,0x02,0x04,0x08,0x08,0x08},
    ['8' - 32] = {0x0E,0x11,0x11,0x0E,0x11,0x11,0x0E},
    ['9' - 32] = {0x0E,0x11,0x11,0x0F,0x01,0x02,0x0C},
    [':' - 32] = {0x00,0x0C,0x0C,0x00,0x0C,0x0C,0x00},
    [';' - 32] = {0x00,0x04,0x00,0x00,0x04,0x04,0x08},
    ['<' - 32] = {0x02,0x04,0x08,0x10,0x08,0x04,0x02},
    ['=' - 32] = {0x00,0x00,0x1F,0x00,0x1F,0x00,0x00},
    ['>' - 32] = {0x08,0x04,0x02,0x01,0x02,0x04,0x08},
    ['?' - 32] = {0x0E,0x11,0x01,0x02,0x04,0x00,0x04},
    ['@' - 32] = {0x0E,0x11,0x17,0x15,0x17,0x10,0x0E},
    ['A' - 32] = {0x0E,0x11,0x11,0x1F,0x11,0x11,0x11},
    ['B' - 32] = {0x1E,0x11,0x11,0x1E,0x11,0x11,0x1E},
    ['C' - 32] = {0x0E,0x11,0x10,0x10,0x10,0x11,0x0E},
    ['D' - 32] = {0x1C,0x12,0x11,0x11,0x11,0x12,0x1C},
    ['E' - 32] = {0x1F,0x10,0x10,0x1E,0x10,0x10,0x1F},
    ['F' - 32] = {0x1F,0x10,0x10,0x1E,0x10,0x10,0x10},
    ['G' - 32] = {0x0E,0x11,0x10,0x17,0x11,0x11,0x0F},
    ['H' - 32] = {0x11,0x11,0x11,0x1F,0x11,0x11,0x11},
    ['I' - 32] = {0x0E,0x04,0x04,0x04,0x04,0x04,0x0E},
    ['J' - 32] = {0x07,0x02,0x02,0x02,0x02,0x12,0x0C},
    ['K' - 32] = {0x11,0x12,0x14,0x18,0x14,0x12,0x11},
    ['L' - 32] = {0x10,0x10,0x10,0x10,0x10,0x10,0x1F},
    ['M' - 32] = {0x11,0x1B,0x15,0x15,0x11,0x11,0x11},
    ['N' - 32] = {0x11,0x19,0x15,0x13,0x11,0x11,0x11},
    ['O' - 32] = {0x0E,0x11,0x11,0x11,0x11,0x11,0x0E},
    ['P' - 32] = {0x1E,0x11,0x11,0x1E,0x10,0x10,0x10},
    ['Q' - 32] = {0x0E,0x11,0x11,0x11,0x15,0x12,0x0D},
    ['R' - 32] = {0x1E,0x11,0x11,0x1E,0x14,0x12,0x11},
    ['S' - 32] = {0x0F,0x10,0x10,0x0E,0x01,0x01,0x1E},
    ['T' - 32] = {0x1F,0x04,0x04,0x04,0x04,0x04,0x04},
    ['U' - 32] = {0x11,0x11,0x11,0x11,0x11,0x11,0x0E},
    ['V' - 32] = {0x11,0x11,0x11,0x11,0x11,0x0A,0x04},
    ['W' - 32] = {0x11,0x11,0x11,0x15,0x15,0x15,0x0A},
    ['X' - 32] = {0x11,0x11,0x0A,0x04,0x0A,0x11,0x11},
    ['Y' - 32] = {0x11,0x11,0x0A,0x04,0x04,0x04,0x04},
    ['Z' - 32] = {0x1F,0x01,0x02,0x04,0x08,0x10,0x1F},
    ['[' - 32] = {0x0E,0x08,0x08,0x08,0x08,0x08,0x0E},
    ['\\' - 32] = {0x10,0x08,0x08,0x04,0x02,0x02,0x01},
    [']' - 32] = {0x0E,0x02,0x02,0x02,0x02,0x02,0x0E},
    ['^' - 32] = {0x04,0x0A,0x11,0x00,0x00,0x00,0x00},
    ['_' - 32] = {0x00,0x00,0x00,0x00,0x00,0x00,0x1F},
};

static void draw_text(int x, int y, const char *s, int scale, int r, int g, int b) {
    int cx = x;
    for (int i = 0; s[i]; i++) {
        char c = s[i];
        if (c >= 'a' && c <= 'z') c -= 32;
        if (c < 32 || c > 95) c = ' ';
        const uint8_t *glyph = font5x7[c - 32];
        for (int row = 0; row < 7; row++) {
            uint8_t bits = glyph[row];
            for (int col = 0; col < 5; col++) {
                if (bits & (1 << (4 - col))) {
                    for (int sy = 0; sy < scale; sy++) {
                        for (int sx = 0; sx < scale; sx++) {
                            clamp_put(cx + col * scale + sx, y + row * scale + sy, r, g, b);
                        }
                    }
                }
            }
        }
        cx += 6 * scale;
    }
}

/* Project 3D world point (X lateral m, Y height m, Z forward distance m) to 2D screen */
static int project3d(float X, float Y, float Z, float cam_x, float cam_y, float cam_z, float roll_deg, float *sx, float *sy) {
    float dz = Z - cam_z;
    if (dz < 2.0f) return 0;
    float dx = X - cam_x;
    float dy = Y - cam_y;
    float rad = roll_deg * 3.14159265f / 180.0f;
    float cr = cosf(rad), sr = sinf(rad);
    float rx = dx * cr - dy * sr;
    float ry = dx * sr + dy * cr;
    float fov = 420.0f;
    *sx = W * 0.5f + (rx / dz) * fov;
    *sy = H * 0.45f - (ry / dz) * fov;
    return 1;
}

/* Base 3D FlightGear Cockpit & Runway 09 Craiova renderer */
static void render_base_cockpit(float cam_x, float cam_y, float cam_z, float roll_deg, float fog_density,
                                int swir_lens_active, int lens_cx, int lens_cy, int lens_rad) {
    float rad = roll_deg * 3.14159265f / 180.0f;
    float sr = sinf(rad), cr = cosf(rad);

    /* Sky & Ground with roll and CAT IIIc fog */
    for (int y = 0; y < H; y++) {
        for (int x = 0; x < W; x++) {
            float nx = (x - W * 0.5f);
            float ny = (y - H * 0.45f);
            float ry = nx * sr + ny * cr;
            int is_lens = 0;
            if (swir_lens_active) {
                int dx = x - lens_cx, dy = y - lens_cy;
                if (dx * dx + dy * dy <= lens_rad * lens_rad) is_lens = 1;
            }
            float eff_fog = is_lens ? (fog_density * 0.22f) : fog_density;

            int r, g, b;
            if (ry < 0) {
                /* Sky */
                float t = (-ry) / (H * 0.55f);
                if (t > 1.0f) t = 1.0f;
                r = (int)(18 + 25 * (1.0f - t));
                g = (int)(28 + 40 * (1.0f - t));
                b = (int)(48 + 55 * (1.0f - t));
            } else {
                /* Ground */
                float t = ry / (H * 0.55f);
                if (t > 1.0f) t = 1.0f;
                r = (int)(22 + 18 * t);
                g = (int)(45 + 30 * t);
                b = (int)(25 + 15 * t);
            }
            if (is_lens) {
                /* Thermal SWIR tint */
                g = (int)(g * 1.25f + 15);
            }
            /* Blend CAT IIIc fog */
            int fr = is_lens ? 35 : 215;
            int fg = is_lens ? 60 : 222;
            int fb_c = is_lens ? 55 : 230;
            fb[y][x][0] = (uint8_t)(r * (1.0f - eff_fog) + fr * eff_fog);
            fb[y][x][1] = (uint8_t)(g * (1.0f - eff_fog) + fg * eff_fog);
            fb[y][x][2] = (uint8_t)(b * (1.0f - eff_fog) + fb_c * eff_fog);
        }
    }

    /* Draw 3D Runway 09 Craiova (Z from 0 to 2200m, width 45m -> X in [-22.5, +22.5]) */
    for (float z = cam_z + 8.0f; z < cam_z + 1400.0f; z += 6.0f) {
        if (z < -400.0f) continue;
        float sxL, syL, sxR, syR;
        float half_w = (z >= 0.0f) ? 24.0f : 3.0f;
        if (project3d(-half_w, 0.0f, z, cam_x, cam_y, cam_z, roll_deg, &sxL, &syL) &&
            project3d(+half_w, 0.0f, z, cam_x, cam_y, cam_z, roll_deg, &sxR, &syR)) {
            if (z >= 0.0f) {
                float dist_atten = expf(-(z - cam_z) * 0.0022f * (0.25f + fog_density));
                draw_line(sxL, syL, sxR, syR, 3, 50, 56, 65, 0.55f * dist_atten + 0.15f);
                /* Runway edge lines */
                draw_line(sxL, syL, sxL + 2, syL, 2, 230, 235, 240, 0.7f * dist_atten);
                draw_line(sxR - 2, syR, sxR, syR, 2, 230, 235, 240, 0.7f * dist_atten);
                /* Centerline dashes */
                if (((int)(z / 25.0f)) % 2 == 0) {
                    float sxC, syC;
                    if (project3d(0.0f, 0.0f, z, cam_x, cam_y, cam_z, roll_deg, &sxC, &syC)) {
                        draw_circle((int)sxC, (int)syC, 2, 1, 245, 245, 250, 0.8f * dist_atten);
                    }
                }
            }
        }
    }

    /* Threshold "Zebra" Stripes at Z = 5..35m */
    for (int stripe = -4; stripe <= 4; stripe++) {
        if (stripe == 0) continue;
        float x_s = stripe * 4.2f;
        float sx0, sy0, sx1, sy1;
        if (project3d(x_s, 0.0f, 8.0f, cam_x, cam_y, cam_z, roll_deg, &sx0, &sy0) &&
            project3d(x_s, 0.0f, 38.0f, cam_x, cam_y, cam_z, roll_deg, &sx1, &sy1)) {
            draw_line(sx0, sy0, sx1, sy1, 3, 240, 255, 240, 0.85f);
        }
    }

    /* Calvert ALS Approach Light Bars (Z = -420m to 0m every 30m) */
    for (float z = -420.0f; z <= 300.0f; z += 30.0f) {
        if (z <= cam_z + 5.0f) continue;
        int is_crossbar = ((int)fabsf(z) % 150 == 0) && (z < 0.0f);
        int n_lights = is_crossbar ? 7 : 3;
        for (int li = -n_lights; li <= n_lights; li++) {
            float lx = li * 2.8f;
            if (z >= 0.0f && fabsf(lx) > 0.1f && fabsf(lx) < 20.0f) continue;
            float sx, sy;
            if (project3d(lx, 0.0f, z, cam_x, cam_y, cam_z, roll_deg, &sx, &sy)) {
                int ix = (int)sx, iy = (int)sy;
                int in_lens = 0;
                if (swir_lens_active) {
                    int dx = ix - lens_cx, dy = iy - lens_cy;
                    if (dx * dx + dy * dy <= lens_rad * lens_rad) in_lens = 1;
                }
                int lr = (z < 0.0f) ? 255 : 180;
                int lg = (z < 0.0f) ? 235 : 255;
                int lb = (z < 0.0f) ? 140 : 180;
                if (z >= -10.0f && z <= 10.0f) { lr = 40; lg = 255; lb = 90; } /* Threshold green */

                if (swir_lens_active && !in_lens && fog_density > 0.7f) {
                    /* Outside SWIR lens in heavy fog: dim Mie bloom halo */
                    draw_circle(ix, iy, 6, 1, 235, 235, 240, 0.25f);
                } else {
                    /* Crisp ALS bulb */
                    draw_circle(ix, iy, in_lens ? 4 : 3, 1, lr, lg, lb, 0.95f);
                    if (in_lens) {
                        /* Sub-pixel crosshair on centroid */
                        draw_line(ix - 5, iy, ix + 5, iy, 1, 0, 255, 255, 1.0f);
                        draw_line(ix, iy - 5, ix, iy + 5, 1, 0, 255, 255, 1.0f);
                    }
                }
            }
        }
    }

    /* Cockpit Glare Shield & Pillars */
    fill_rect(0, H - 38, W, 38, 14, 18, 24, 0.94f);
    draw_line(0, H - 38, W, H - 38, 2, 0, 210, 180, 0.8f);
    draw_line(0, 0, 55, H - 38, 8, 20, 25, 32, 0.9f);
    draw_line(W - 1, 0, W - 56, H - 38, 8, 20, 25, 32, 0.9f);

    /* Top Telemetry Banner */
    fill_rect(0, 0, W, 32, 10, 15, 24, 0.88f);
    draw_line(0, 32, W, 32, 1, 0, 220, 255, 0.8f);
}

static void render_lesson(int lesson, const char *out_path) {
    char cmd[1024];
    snprintf(cmd, sizeof(cmd),
        "ffmpeg -y -loglevel error -f rawvideo -pix_fmt rgb24 -s %dx%d -r %d -i - "
        "-c:v libx264 -preset ultrafast -crf 22 -pix_fmt yuv420p \"%s\"",
        W, H, FPS, out_path);
    FILE *pipe = popen(cmd, "w");
    if (!pipe) { perror("popen"); exit(1); }

    for (int f = 0; f < FRAMES; f++) {
        float t = (float)f / FPS;       /* 0.0 .. 8.0s */
        float prog = (float)f / FRAMES; /* 0.0 .. 1.0 */

        if (lesson == 1) {
            /* Lesson 1: FG-01/FG-04 CAT IIIc Fog + Spoofing +550m + Golden Scissors + Green Path */
            float fog = (t < 2.0f) ? (0.25f + 0.62f * (t / 2.0f)) : 0.87f;
            float cam_z = -380.0f + prog * 310.0f;
            float cam_y = 65.0f - prog * 45.0f;
            float cam_x = (t > 2.0f && t < 4.8f) ? (12.0f * sinf((t - 2.0f) * 1.1f)) : 0.0f;
            render_base_cockpit(cam_x, cam_y, cam_z, 0.0f, fog, 0, 0, 0, 0);

            draw_text(14, 9, "FG-01/04 LRCV RWY 09 CRAIOVA: CAT IIIc (V_MOR=30m) + SPOOFING +550m DEFENSE", 2, 0, 245, 255);

            /* Red Spoofed Trajectory pulling right (+550m toward trees/field) */
            if (t >= 1.8f) {
                for (int i = 0; i < 18; i++) {
                    float z0 = cam_z + 25.0f + i * 18.0f;
                    float z1 = z0 + 14.0f;
                    float x0 = 0.08f * (i * i) * 1.6f;
                    float x1 = 0.08f * ((i + 1) * (i + 1)) * 1.6f;
                    float sx0, sy0, sx1, sy1;
                    if (project3d(x0, 0.0f, z0, cam_x, cam_y, cam_z, 0.0f, &sx0, &sy0) &&
                        project3d(x1, 0.0f, z1, cam_x, cam_y, cam_z, 0.0f, &sx1, &sy1)) {
                        draw_line(sx0, sy0, sx1, sy1, 3, 255, 45, 55, (t > 4.8f) ? 0.35f : 0.95f);
                    }
                }
                fill_rect(460, 48, 325, 42, 45, 10, 15, 0.85f);
                draw_text(470, 55, "HACKER GPS SPOOFING: +550m LATERAL", 1, 255, 80, 80);
                draw_text(470, 70, "FALSE PATH -> TREES / FIELD DANGER!", 1, 255, 180, 80);
            }

            /* Golden Scissors cutting GPS wire + Green AERO-SHIELD Path */
            if (t >= 4.2f) {
                for (int i = 0; i < 18; i++) {
                    float z0 = cam_z + 20.0f + i * 18.0f;
                    float z1 = z0 + 14.0f;
                    float sx0, sy0, sx1, sy1;
                    if (project3d(0.0f, 0.0f, z0, cam_x, cam_y, cam_z, 0.0f, &sx0, &sy0) &&
                        project3d(0.0f, 0.0f, z1, cam_x, cam_y, cam_z, 0.0f, &sx1, &sy1)) {
                        draw_line(sx0, sy0, sx1, sy1, 4, 0, 255, 110, 0.95f);
                    }
                }
                /* Golden Scissors Icon */
                int scx = 490, scy = 145;
                draw_circle(scx - 10, scy - 8, 8, 0, 255, 215, 0, 1.0f);
                draw_circle(scx - 10, scy + 8, 8, 0, 255, 215, 0, 1.0f);
                draw_line(scx - 2, scy - 6, scx + 24, scy + 10, 3, 255, 215, 0, 1.0f);
                draw_line(scx - 2, scy + 6, scx + 24, scy - 10, 3, 255, 215, 0, 1.0f);
                fill_rect(16, 48, 425, 46, 8, 40, 24, 0.88f);
                draw_text(26, 55, "GOLDEN SCISSORS: FALSE GPS WIRE CUT!", 2, 255, 220, 40);
                draw_text(26, 76, "AERO-SHIELD + DROSOPHILA -> RWY 09 (0.22m)", 1, 50, 255, 130);
            }
            draw_text(18, H - 25, "ALT: 60m->15m | VIS: V_MOR=30m CAT IIIc | STATUS: AERO-SHIELD LOCKED RWY 09", 1, 180, 245, 220);

        } else if (lesson == 2) {
            /* Lesson 2: FG-02 T4/T5 SWIR Infrared Magnifying Lens sweeping over fog wall */
            float cam_z = -340.0f + prog * 260.0f;
            float cam_y = 58.0f - prog * 30.0f;
            int lcx = (int)(W * 0.5f + 170.0f * sinf(t * 1.35f));
            int lcy = (int)(H * 0.52f + 35.0f * cosf(t * 1.8f));
            int lrad = 105;
            render_base_cockpit(0.0f, cam_y, cam_z, 0.0f, 0.92f, 1, lcx, lcy, lrad);

            /* Draw SWIR Magnifying Lens Rim & Handle */
            draw_circle(lcx, lcy, lrad, 0, 0, 255, 200, 1.0f);
            draw_circle(lcx, lcy, lrad - 2, 0, 255, 220, 40, 1.0f);
            draw_line(lcx + lrad * 0.7f, lcy + lrad * 0.7f, lcx + lrad * 1.15f, lcy + lrad * 1.15f, 6, 255, 200, 50, 1.0f);

            draw_text(14, 9, "FG-02 SWIR INFRARED LENS (0.9-14um) + T4/T5 EYE: MIE BLOOM SUPPRESSED", 2, 0, 255, 200);
            fill_rect(16, 44, 355, 42, 12, 22, 32, 0.85f);
            draw_text(24, 51, "OUTSIDE LENS: CAT IIIc FOG WALL (beta_VIS)", 1, 255, 120, 120);
            draw_text(24, 66, "INSIDE LENS: beta_SWIR = 0.22 * beta_VIS | M_bloom = 0", 1, 80, 255, 180);
            draw_text(lcx - 85, lcy - lrad - 14, "SWIR 0.9-14um + T4/T5 SUB-PIXEL (+)", 1, 255, 240, 80);
            draw_text(18, H - 25, "ALS CENTROIDS DETECTED: 24/24 | MIE HALO MASK: M_bloom=0 | SUB-PIXEL LOCK: 0.08 px", 1, 180, 245, 220);

        } else if (lesson == 3) {
            /* Lesson 3: FG-03 Optical Odometry from ALS Bulbs (NO GPS) + Yellow Ruler + E-PG 16-LED Ring */
            float cam_z = -320.0f + prog * 280.0f;
            float cam_y = 52.0f - prog * 28.0f;
            render_base_cockpit(0.0f, cam_y, cam_z, 0.0f, 0.35f, 0, 0, 0, 0);

            draw_text(14, 9, "FG-03 ALS OPTICAL ODOMETRY (NO GPS) + 16-NEURON E-PG COMPASS RING", 2, 80, 255, 140);

            /* Green Optical Flow Vectors on ALS lights */
            for (float z = -270.0f; z <= 60.0f; z += 30.0f) {
                if (z <= cam_z + 15.0f) continue;
                for (int li = -2; li <= 2; li += 2) {
                    float lx = li * 3.5f;
                    float sx0, sy0, sx1, sy1;
                    if (project3d(lx, 0.0f, z, 0.0f, cam_y, cam_z, 0.0f, &sx0, &sy0) &&
                        project3d(lx, 0.0f, z - 12.0f, 0.0f, cam_y, cam_z, 0.0f, &sx1, &sy1)) {
                        draw_arrow(sx0, sy0, sx1, sy1, 2, 0, 255, 100);
                    }
                }
            }

            /* Yellow Ruler measuring delta_u between frames */
            int rx = 180, ry = 250, rw = 260, rh = 24;
            fill_rect(rx, ry, rw, rh, 245, 210, 25, 0.92f);
            for (int tick = 0; tick <= rw; tick += 10) {
                int th = (tick % 50 == 0) ? 10 : 5;
                draw_line(rx + tick, ry, rx + tick, ry + th, 1, 20, 20, 20, 1.0f);
            }
            draw_text(rx + 10, ry + 12, "RULER delta_u -> V_ground = 68.0 m/s", 1, 15, 15, 15);

            /* E-PG Compass Ring with 16 LEDs in top-right corner */
            int ecx = 695, ecy = 115, erad = 52;
            fill_rect(615, 42, 165, 152, 12, 18, 28, 0.88f);
            draw_text(632, 48, "E-PG COMPASS (16)", 1, 0, 240, 255);
            draw_circle(ecx, ecy, erad, 0, 60, 110, 140, 1.0f);
            for (int k = 0; k < 16; k++) {
                float a = k * (2.0f * 3.14159265f / 16.0f) - 1.5708f;
                int nx = ecx + (int)(erad * cosf(a));
                int ny = ecy + (int)(erad * sinf(a));
                int active = (k == 4 || k == 3 || k == 5); /* East 090 deg bump */
                draw_circle(nx, ny, active ? 6 : 4, 1, active ? 0 : 45, active ? 255 : 90, active ? 120 : 110, 1.0f);
            }
            draw_arrow(ecx, ecy, ecx + 34, ecy, 2, 0, 255, 120);
            draw_text(635, 178, "HDG: 090.0 deg RWY09", 1, 80, 255, 140);
            draw_text(18, H - 25, "OPTICAL ODOMETRY V_ground: 68.02 m/s (ERR 0.14 m/s) | GPS: OFF | E-PG: LOCKED", 1, 180, 245, 220);

        } else if (lesson == 4) {
            /* Lesson 4: FG-06/FG-08 6-DoF Roll +22 deg + 18 m/s Fog Gust + 2-Layer Separation */
            float roll = 22.0f * sinf(t * 0.9f);
            float cam_z = -310.0f + prog * 250.0f;
            float cam_y = 55.0f - prog * 25.0f;
            render_base_cockpit(0.0f, cam_y, cam_z, roll, 0.65f, 0, 0, 0, 0);

            draw_text(14, 9, "FG-06/08 6-DoF ROLL +22 deg & 18 m/s FOG GUST: 2-LAYER FLOW SEPARATION", 2, 255, 210, 60);

            /* Upper Layer: Purple Fog Gust Arrows (w_fog = 18 m/s) */
            fill_rect(18, 42, 430, 38, 45, 15, 55, 0.85f);
            draw_text(26, 48, "LAYER 1 (TOP): PURPLE FOG GUST w_fog = 18.0 m/s", 1, 235, 130, 255);
            draw_text(26, 62, "PFNd ALERT: GUST LOOKAHEAD 0.85s BEFORE IMPACT!", 1, 255, 230, 60);
            for (int y = 95; y <= 155; y += 30) {
                for (int x = 90; x <= 680; x += 110) {
                    int shift = (int)(f * 4) % 40;
                    draw_arrow(x + shift, y, x + shift + 48, y + 6, 2, 215, 80, 255);
                }
            }

            /* Lower Layer: Green HS/VS Derotated Ground Arrows (B_rot * omega_k) */
            fill_rect(18, 265, 465, 38, 10, 42, 25, 0.88f);
            draw_text(26, 271, "LAYER 2 (BOTTOM): HS/VS DEROTATED GROUND (B_rot * omega_k)", 1, 60, 255, 130);
            draw_text(26, 285, "INVARIANT TO +22 deg BANK ANGLE -> PURE RUNWAY TRACK!", 1, 180, 255, 200);
            for (int x = 160; x <= 640; x += 95) {
                draw_arrow(x, 210, x + (x - 400) * 0.18f, 252, 2, 0, 255, 110);
            }
            draw_text(18, H - 25, "BANK ANGLE: +22.0 deg | FOG GUST: 18.0 m/s | PFNd LOOKAHEAD: 0.85s | TRACK: 0.24m", 1, 180, 245, 220);

        } else if (lesson == 5) {
            /* Lesson 5: FG-05 Stealth Spoofing 0.22 m/s caught by Scale of Truth & CUSUM Glass in 2.9s */
            float cam_z = -330.0f + prog * 260.0f;
            float cam_y = 54.0f - prog * 28.0f;
            render_base_cockpit(0.0f, cam_y, cam_z, 0.0f, 0.55f, 0, 0, 0, 0);

            draw_text(14, 9, "FG-05 STEALTH SPOOFING (0.22 m/s) CAUGHT BY CUSUM GLASS IN 2.9s", 2, 255, 195, 50);

            /* Stealth Red GPS Drift vs Green True Centerline */
            for (int i = 0; i < 15; i++) {
                float z0 = cam_z + 20.0f + i * 18.0f;
                float z1 = z0 + 14.0f;
                float drift = (t * 2.2f) * (i / 15.0f);
                float sx0, sy0, sx1, sy1;
                if (project3d(drift, 0.0f, z0, 0.0f, cam_y, cam_z, 0.0f, &sx0, &sy0) &&
                    project3d(drift + 0.6f, 0.0f, z1, 0.0f, cam_y, cam_z, 0.0f, &sx1, &sy1)) {
                    draw_line(sx0, sy0, sx1, sy1, 2, 255, 70, 70, 0.9f);
                }
                if (project3d(0.0f, 0.0f, z0, 0.0f, cam_y, cam_z, 0.0f, &sx0, &sy0) &&
                    project3d(0.0f, 0.0f, z1, 0.0f, cam_y, cam_z, 0.0f, &sx1, &sy1)) {
                    draw_line(sx0, sy0, sx1, sy1, 3, 0, 255, 110, 0.95f);
                }
            }

            /* Left Widget: Scale of Truth (GPS vs T4/T5 Eye + Wings) */
            fill_rect(18, 48, 255, 135, 14, 20, 30, 0.88f);
            draw_text(28, 55, "SCALE OF TRUTH (INS+T4/T5)", 1, 0, 235, 255);
            int bx = 145, by = 115;
            float tilt = (t < 2.9f) ? (t / 2.9f) * 0.35f : 0.35f;
            draw_line(bx, by + 35, bx, by, 3, 200, 200, 210, 1.0f);
            draw_line(bx - 65, by - 65 * sinf(tilt), bx + 65, by + 65 * sinf(tilt), 3, 255, 215, 0, 1.0f);
            draw_text(35, 155, "GPS (FAKE)", 1, 255, 80, 80);
            draw_text(155, 155, "T4/T5 + WINGS", 1, 60, 255, 120);

            /* Right Widget: CUSUM Glass filling up in 2.9s */
            fill_rect(535, 48, 245, 215, 14, 20, 30, 0.90f);
            draw_text(548, 55, "CUSUM ALARM GLASS (2.9s)", 1, 255, 220, 60);
            int gx = 585, gy = 85, gw = 70, gh = 130;
            draw_line(gx, gy, gx, gy + gh, 3, 220, 230, 240, 1.0f);
            draw_line(gx + gw, gy, gx + gw, gy + gh, 3, 220, 230, 240, 1.0f);
            draw_line(gx, gy + gh, gx + gw, gy + gh, 3, 220, 230, 240, 1.0f);
            float fill_ratio = t / 2.9f;
            if (fill_ratio > 1.0f) fill_ratio = 1.0f;
            int fh = (int)(fill_ratio * 105.0f);
            int caught = (t >= 2.9f);
            fill_rect(gx + 3, gy + gh - fh - 2, gw - 6, fh, caught ? 255 : 255, caught ? 45 : 185, 30, 0.88f);
            /* Threshold line h_CUSUM */
            draw_line(gx - 10, gy + 25, gx + gw + 10, gy + 25, 2, 255, 60, 60, 1.0f);
            draw_text(gx + gw + 14, gy + 22, "h_ALARM", 1, 255, 80, 80);
            if (caught) {
                fill_rect(285, 55, 238, 42, 60, 10, 15, 0.92f);
                draw_text(295, 63, "STEALTH SPOOFING BLOCKED!", 1, 255, 230, 50);
                draw_text(295, 78, "DETECTED AT t = 2.90s", 1, 80, 255, 140);
            }
            draw_text(18, H - 25, "STEALTH RAMP: 0.22 m/s | CUSUM S_k > h AT 2.9s | FALSE ALARM RATE: 0.0%", 1, 180, 245, 220);

        } else {
            /* Lesson 6: FG-10 Full Autonomous CAT IIIc Landing RWY 09 (300m -> Touchdown Zebra) + 4-Aircraft Race */
            float cam_z = -410.0f + prog * 435.0f; /* Crosses threshold Z=0 at touchdown! */
            float cam_y = 75.0f * (1.0f - prog * 0.95f);
            render_base_cockpit(0.0f, cam_y, cam_z, 0.0f, 0.48f, 0, 0, 0, 0);

            draw_text(14, 9, "FG-10 AUTONOMOUS CAT IIIc TOUCHDOWN RWY 09 CRAIOVA + 4-AIRCRAFT RACE", 2, 0, 255, 140);

            /* Green Centerline Landing Guidance Tunnel */
            for (int i = 1; i <= 8; i++) {
                float zt = cam_z + i * 35.0f;
                if (zt < 0.0f) continue;
                float sxL, syL, sxR, syR;
                if (project3d(-12.0f, 0.0f, zt, 0.0f, cam_y, cam_z, 0.0f, &sxL, &syL) &&
                    project3d(+12.0f, 0.0f, zt, 0.0f, cam_y, cam_z, 0.0f, &sxR, &syR)) {
                    draw_line(sxL, syL, sxR, syR, 2, 0, 255, 120, 0.85f);
                }
            }

            /* 4-Aircraft Race Mini-Panel on Right */
            int px = 495, py = 44, pw = 290, ph = 155;
            fill_rect(px, py, pw, ph, 10, 16, 26, 0.90f);
            draw_text(px + 10, py + 8, "4-AIRCRAFT RACE TO RWY 09:", 1, 255, 230, 80);
            /* Mini runway centerline */
            fill_rect(px + 20, py + 115, pw - 40, 22, 45, 52, 62, 1.0f);
            draw_line(px + 20, py + 126, px + pw - 20, py + 126, 1, 240, 240, 240, 1.0f);

            int rx_end = (int)(px + 30 + prog * (pw - 70));
            /* 1. Raw GNSS (Red -> crashes in field) */
            draw_line(px + 30, py + 126, rx_end, py + 126 - (int)(prog * 85), 2, 255, 55, 55, 1.0f);
            draw_text(px + 12, py + 24, "1. RAW GNSS: +550m CRASH", 1, 255, 75, 75);
            /* 2. Pure INS (Orange -> drifts 58.4m) */
            draw_line(px + 30, py + 126, rx_end, py + 126 - (int)(prog * 55), 2, 255, 155, 40, 1.0f);
            draw_text(px + 12, py + 38, "2. PURE INS: 58.4m DRIFT", 1, 255, 165, 50);
            /* 3. Naive Flow (Yellow -> blown by wind 34.2m) */
            draw_line(px + 30, py + 126, rx_end, py + 126 - (int)(prog * 32), 2, 255, 230, 50, 1.0f);
            draw_text(px + 12, py + 52, "3. NAIVE FLOW: 34.2m ERR", 1, 255, 235, 60);
            /* 4. AERO-SHIELD + Drosophila (Green -> 0.22m on RWY 09!) */
            draw_line(px + 30, py + 126, rx_end, py + 126, 3, 0, 255, 110, 1.0f);
            draw_circle(rx_end, py + 126, 5, 1, 0, 255, 110, 1.0f);
            draw_text(px + 12, py + 68, "4. AERO-SHIELD: 0.22m OK!", 1, 50, 255, 130);

            if (prog > 0.78f) {
                fill_rect(22, 48, 390, 44, 8, 48, 26, 0.92f);
                draw_text(32, 56, "TOUCHDOWN RWY 09 ZEBRA! ERR = 0.22m", 2, 50, 255, 130);
            }
            draw_text(18, H - 25, "ALT: 300m->0m TOUCHDOWN | LATERAL ERR: 0.22m (<0.30m SPEC) | CAT IIIc AUTOLAND OK", 1, 180, 245, 220);
        }

        fwrite(fb, 1, sizeof(fb), pipe);
    }
    pclose(pipe);
}

int main(int argc, char **argv) {
    if (argc < 3) return 1;
    int lesson = atoi(argv[1]);
    render_lesson(lesson, argv[2]);
    return 0;
}
"""

def main():
    t0 = time.time()
    c_file = os.path.join(OUT_DIR, "fg_renderer.c")
    bin_file = os.path.join(OUT_DIR, "fg_renderer_bin")
    with open(c_file, "w") as f:
        f.write(C_SOURCE)

    subprocess.run(["gcc", "-O3", c_file, "-lm", "-o", bin_file], check=True)

    procs = []
    for i in range(1, 7):
        out_mp4 = os.path.join(OUT_DIR, f"fg_lesson_{i}.mp4")
        p = subprocess.Popen([bin_file, str(i), out_mp4])
        procs.append((i, out_mp4, p))

    for i, out_mp4, p in procs:
        rc = p.wait()
        if rc != 0:
            raise RuntimeError(f"Failed generating {out_mp4}")
        print(f"Generated {out_mp4} ({os.path.getsize(out_mp4)} bytes)")

    os.remove(c_file)
    os.remove(bin_file)
    print(f"All 6 videos generated in {time.time() - t0:.2f}s!")

if __name__ == "__main__":
    main()
