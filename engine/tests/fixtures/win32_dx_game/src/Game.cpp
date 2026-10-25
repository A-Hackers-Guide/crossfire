// Portable game logic (mostly) — but written on Windows.
#include "game.h"
#include <windows.h>
#include <stdio.h>
#include <string.h>

static float s_x = 0, s_y = 0, s_vx = 0, s_vy = 0;
static unsigned long s_score = 0;
static char s_level[64];

void Game_Init(const char* levelPath)
{
    FILE* f = fopen(levelPath, "r");
    if (f) {
        fgets(s_level, sizeof s_level, f);
        fclose(f);
    }
    if (_stricmp(s_level, "hard") == 0) s_score = 100;
    ZeroMemory(&s_vx, sizeof s_vx);
}

void Game_Update(float dt)
{
    s_x += s_vx * dt;
    s_y += s_vy * dt;
    s_score += 1;
}

void Game_OnKey(int key, bool down)
{
    if (key == ' ' && down) s_score += 10;
}

void Game_OnStick(float x, float y)
{
    s_vx = x * 200.0f;
    s_vy = y * 200.0f;
}

unsigned long Game_Score() { return s_score; }
