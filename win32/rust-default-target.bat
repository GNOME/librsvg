@echo off
:: SETLOCAL ENABLEDELAYEDEXPANSION
set RUSTUP=%1

FOR /F "tokens=* USEBACKQ" %%F IN (`%RUSTUP% default`) DO (
SET RUST_DEFAULT_TOOLCHAIN=%%F
)

:: We want to be very sure that if we are using the default
:: Rust toolchain for the build, we are indeed using an msvc
:: one!

FOR /F "tokens=1,2,3,4,5*  delims=-" %%a IN ("%RUST_DEFAULT_TOOLCHAIN%") do (
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
echo RUST_DEFAULT_TARGET=%NMAKE_TGT%>>rust-cfg.mak
echo RUST_DEFAULT_COMPILER=%TOOLCHAIN_COMPILER%>>rust-cfg.mak
