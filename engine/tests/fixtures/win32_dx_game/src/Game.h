#pragma once

void Game_Init(const char* levelPath);
void Game_Update(float dt);
void Game_OnKey(int key, bool down);
void Game_OnStick(float x, float y);
unsigned long Game_Score();
