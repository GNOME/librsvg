@echo off
if not "%1" == "use-rustup" if not "%1" == "check-syslibs" goto :err_badopt
if "%2" == "" goto :err_badopt

set RUSTUP=%2
if "%1" == "check-syslibs" goto :check_syslibs

FOR /F "tokens=* USEBACKQ" %%F IN (`%RUSTUP% default`) DO (
SET RUST_DEFAULT_TOOLCHAIN=%%F
)

:: We want to be very sure that if we are using the default
:: Rust toolchain for the build, we are indeed using an msvc
:: one!

FOR /F "tokens=1,2,3,4,5* delims=-" %%a IN ("%RUST_DEFAULT_TOOLCHAIN%") do (
:: <version>-<platform>-pc-windows-msvc (default) or  stable-<platform>-pc-windows-msvc (default)
if not "%%a" == "nightly" set CHANNEL=%%a
if not "%%a" == "nightly" set TARGET=%%b
if not "%%a" == "nightly" FOR /F "tokens=1,2 delims= " %%o IN ("%%e") do (
set TOOLCHAIN_COMPILER=%%c-%%d-%%o
)

:: nightly-yyyy-mm-dd-<platform>-pc-windows-msvc (default)
if "%%a" == "nightly" set CHANNEL=%%a-%%b-%%c
if "%%a" == "nightly" set TARGET=%%e
if "%%a" == "nightly" FOR /F "tokens=1,2 delims= " %%o IN ("%%f") do (
set TOOLCHAIN_COMPILER=%%o
)
)

if "%TARGET%" == "aarch64" set NMAKE_TGT=amd64
if "%TARGET%" == "x86_64" set NMAKE_TGT=x64
if "%TARGET%" == "i686" set NMAKE_TGT=Win32

if exist rust-cfg.mak goto :EOF
echo RUST_DEFAULT_CHANNEL=%CHANNEL%>>rust-cfg.mak
echo RUST_DEFAULT_TARGET=%TARGET%>>rust-cfg.mak
echo RUST_DEFAULT_MSVC_TARGET=%NMAKE_TGT%>>rust-cfg.mak
echo RUST_DEFAULT_COMPILER=%TOOLCHAIN_COMPILER%>>rust-cfg.mak
goto :EOF

:check_syslibs
if "%3" == "" goto :err_badopt
if "%4" == "" goto :err_badopt
if not "%4" == "x86_64" if not "%4" == "aarch64" if not "%4" == "i686" goto :err_badopt

:: grab the results from the following command to extract the needed system
:: libs for linking for Rust builds (this is printed in stderr)
if exist rust-libs.txt goto :check_libs
%RUSTUP:rustup=rustc% %3 --target=%4-pc-windows-msvc ^
--crate-type staticlib --print native-static-libs - < nul 2>>rust-libs.txt

:check_libs
for /f "tokens=1,2*" %%l in ('findstr /ic:"note: native-static-libs:" /b rust-libs.txt') DO (
set ALL_SYS_LIBS=%%n
)
SETLOCAL ENABLEDELAYEDEXPANSION
set LINK_SYS_LIBS=
del /f/q rust-libs.txt
del /f/q rust_out.lib
:: Filter out kernel32.lib, msvcrt.lib and legacy_stdio_definitions.lib
:: they will be pulled in automatically
for %%q in (%ALL_SYS_LIBS%) do (
if not "%%q" == "kernel32.lib" if not "%%q" == "msvcrt.lib" ^
if not "%%q" == "legacy_stdio_definitions.lib" ^
if not "!LINK_SYS_LIBS!" == "" set LINK_SYS_LIBS=!LINK_SYS_LIBS! %%q

if not "%%q" == "kernel32.lib" if not "%%q" == "msvcrt.lib" ^
if not "%%q" == "legacy_stdio_definitions.lib" ^
if "!LINK_SYS_LIBS!" == "" set LINK_SYS_LIBS=%%q
)

echo LIBRSVG_SYSTEM_DEP_LIBS=%LINK_SYS_LIBS%>rust-sys-libs.mak
goto :EOF

:err_badopt
echo Usage: %0 [use-rustup^|check-syslibs] rustup-executable ^<rust-toolchain-for-check-syslibs^> ^<rust-target-platform-for-check-syslibs^>
