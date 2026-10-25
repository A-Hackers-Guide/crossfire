@echo off
call "%ProgramFiles%\Microsoft Visual Studio\2022\Community\VC\Auxiliary\Build\vcvars64.bat"
msbuild AsteroidDrift.sln /p:Configuration=Release
xcopy /y assets bin\Release\assets\
