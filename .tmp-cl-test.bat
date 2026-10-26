@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat" >nul
cd /d "%USERPROFILE%\.cargo\registry\src\rsproxy.cn-e3de039b2554c837\ring-0.17.7"
set OUT=d:\compil_world_view_cache\target\debug\build\ring-2e45439bd3ba0797\out
echo === with -I OUT ===
"C:\Program Files\Microsoft Visual Studio\18\Community\VC\Tools\MSVC\14.51.36231\bin\HostX64\x64\cl.exe" -nologo -MD -Z7 -Brepro -I include -I "%OUT%" -W4 /Gy /Zc:wchar_t /Zc:forScope /Zc:inline /Wall /wd4127 /wd4464 /wd4514 /wd4710 /wd4711 /wd4820 /wd5045 -DNDEBUG -Fo"%TEMP%\mem_manual.o" -c crypto/mem.c
echo EXITCODE=%ERRORLEVEL%
echo === nasm / perl present? ===
where nasm
where perl
