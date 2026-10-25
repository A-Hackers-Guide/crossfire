// Asteroid Drift — a tiny Win32 + Direct3D 11 game.
#include <Windows.h>
#include <d3d11.h>
#include <dxgi.h>
#include <d3dcompiler.h>
#include <Xinput.h>
#include <stdio.h>
#include "Game.h"

#pragma comment(lib, "d3d11.lib")
#pragma comment(lib, "dxgi.lib")
#pragma comment(lib, "d3dcompiler.lib")
#pragma comment(lib, "xinput.lib")

static ID3D11Device* g_device = nullptr;
static ID3D11DeviceContext* g_context = nullptr;
static IDXGISwapChain* g_swapChain = nullptr;
static ID3D11RenderTargetView* g_rtv = nullptr;
static bool g_running = true;

LRESULT CALLBACK WndProc(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
    switch (msg) {
    case WM_KEYDOWN:
        if (wParam == VK_ESCAPE) PostQuitMessage(0);
        Game_OnKey((int)wParam, true);
        return 0;
    case WM_KEYUP:
        Game_OnKey((int)wParam, false);
        return 0;
    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProc(hwnd, msg, wParam, lParam);
}

static bool InitD3D(HWND hwnd)
{
    DXGI_SWAP_CHAIN_DESC sd = {};
    sd.BufferCount = 1;
    sd.BufferDesc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
    sd.BufferUsage = DXGI_USAGE_RENDER_TARGET_OUTPUT;
    sd.OutputWindow = hwnd;
    sd.SampleDesc.Count = 1;
    sd.Windowed = TRUE;
    HRESULT hr = D3D11CreateDeviceAndSwapChain(nullptr, D3D_DRIVER_TYPE_HARDWARE, nullptr, 0, nullptr, 0,
        D3D11_SDK_VERSION, &sd, &g_swapChain, &g_device, nullptr, &g_context);
    if (FAILED(hr)) {
        MessageBoxA(hwnd, "Could not create a Direct3D 11 device.", "Asteroid Drift", MB_OK | MB_ICONERROR);
        return false;
    }
    ID3D11Texture2D* back = nullptr;
    g_swapChain->GetBuffer(0, __uuidof(ID3D11Texture2D), (void**)&back);
    g_device->CreateRenderTargetView(back, nullptr, &g_rtv);
    back->Release();

    ID3DBlob* vs = nullptr;
    D3DCompileFromFile(L"shaders\\basic.hlsl", nullptr, nullptr, "VSMain", "vs_5_0", 0, 0, &vs, nullptr);
    return true;
}

static void PollGamepad()
{
    XINPUT_STATE state = {};
    if (XInputGetState(0, &state) == ERROR_SUCCESS) {
        Game_OnStick(state.Gamepad.sThumbLX / 32768.0f, state.Gamepad.sThumbLY / 32768.0f);
        if (state.Gamepad.wButtons & XINPUT_GAMEPAD_A) Game_OnKey(' ', true);
    }
}

static void SaveHighScore(DWORD score)
{
    HKEY key;
    if (RegCreateKeyExA(HKEY_CURRENT_USER, "Software\\AsteroidDrift", 0, nullptr, 0, KEY_WRITE, nullptr, &key, nullptr) == ERROR_SUCCESS) {
        RegSetValueExA(key, "HighScore", 0, REG_DWORD, (const BYTE*)&score, sizeof(score));
        RegCloseKey(key);
    }
    char path[MAX_PATH];
    snprintf(path, sizeof path, "%s\\AsteroidDrift\\scores.txt", getenv("APPDATA"));
    FILE* f = fopen(path, "w");
    if (f) { fprintf(f, "%lu\n", (unsigned long)score); fclose(f); }
}

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE, LPSTR, int nCmdShow)
{
    WNDCLASSEXA wc = { sizeof(WNDCLASSEXA) };
    wc.lpfnWndProc = WndProc;
    wc.hInstance = hInstance;
    wc.hCursor = LoadCursor(nullptr, IDC_ARROW);
    wc.lpszClassName = "AsteroidDrift";
    RegisterClassExA(&wc);
    HWND hwnd = CreateWindowExA(0, "AsteroidDrift", "Asteroid Drift", WS_OVERLAPPEDWINDOW,
        CW_USEDEFAULT, CW_USEDEFAULT, 1280, 720, nullptr, nullptr, hInstance, nullptr);
    ShowWindow(hwnd, nCmdShow);
    if (!InitD3D(hwnd)) return 1;

    LARGE_INTEGER freq, last, now;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&last);
    Game_Init("assets\\levels\\level1.txt");
    DWORD start = GetTickCount();

    MSG msg = {};
    while (g_running) {
        while (PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE)) {
            if (msg.message == WM_QUIT) g_running = false;
            TranslateMessage(&msg);
            DispatchMessage(&msg);
        }
        QueryPerformanceCounter(&now);
        float dt = (float)(now.QuadPart - last.QuadPart) / (float)freq.QuadPart;
        last = now;
        PollGamepad();
        Game_Update(dt);
        const float clear[4] = { 0.02f, 0.02f, 0.08f, 1.0f };
        g_context->ClearRenderTargetView(g_rtv, clear);
        g_swapChain->Present(1, 0);
        Sleep(1);
    }
    SaveHighScore(Game_Score() + (GetTickCount() - start) / 1000);
    return 0;
}
