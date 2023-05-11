!ifndef VCVER
!include detectenv-msvc.mak
!ifndef LIBDIR
LIBDIR=$(PREFIX)\lib
!endif
!endif

!if "$(CARGO)" == ""
CARGO = cargo
!endif

!if "$(RUSTUP)" == ""
RUSTUP = rustup
!endif

# For those who wish to use the nightly toolchain to build librsvg
!ifdef USE_NIGHTLY_TOOLCHAIN
TOOLCHAIN_TYPE = nightly
!else
TOOLCHAIN_TYPE = stable
!endif

!ifdef VERBOSE
RUST_VERBOSE_FLAG = --verbose
!endif

# Use Rust's cross compiling capabilities?
!ifndef FORCE_CROSS
FORCE_CROSS = 0
!endif

!if "$(PLAT)" == "x64"
RUST_TARGET = x86_64
!if "$(PROCESSOR_ARCHITECTURE)" == "ARM64" || "$(PROCESSOR_ARCHITECTURE)" == "x86"
FORCE_CROSS = 1
!endif
!elseif "$(PLAT)" == "arm64"
FORCE_CROSS = 1
RUST_TARGET = aarch64
!else
RUST_TARGET = i686
!endif

!if "$(VALID_CFGSET)" == "TRUE"
BUILD_RUST = 1
!else
BUILD_RUST = 0
!endif

!if "$(BUILD_RUST)" == "1"

CARGO_TARGET = $(RUST_TARGET)-pc-windows-msvc
CARGO_TARGET_CMD = --target $(CARGO_TARGET)
CARGO_TARGET_TOOLCHAIN = $(TOOLCHAIN_TYPE)-$(CARGO_TARGET)
RUSTUP_CMD = $(RUSTUP) default $(DEFAULT_TARGET)
CARGO_TARGET_DIR = vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_c_api
CARGO_TARGET_DIR_FLAG = --target-dir=$(CARGO_TARGET_DIR)

MANIFEST_PATH_FLAG = --manifest-path=..\Cargo.toml
!if $(FORCE_CROSS) > 0
CARGO_CMD = $(CARGO) --locked build $(CARGO_TARGET_CMD) $(MANIFEST_PATH_FLAG) $(CARGO_TARGET_DIR_FLAG)
CARGO_CLEAN_CMD = $(CARGO) clean $(CARGO_TARGET_CMD) $(MANIFEST_PATH_FLAG) $(CARGO_TARGET_DIR_FLAG)
CARGO_TARGET_OUTPUT_DIR = $(CARGO_TARGET_DIR)\$(CARGO_TARGET)\$(CFG)
!else
CARGO_CMD = $(CARGO) +$(CARGO_TARGET_TOOLCHAIN) --locked build $(MANIFEST_PATH_FLAG) $(CARGO_TARGET_DIR_FLAG)
CARGO_CLEAN_CMD = $(CARGO) +$(CARGO_TARGET_TOOLCHAIN) clean $(MANIFEST_PATH_FLAG) $(CARGO_TARGET_DIR_FLAG)
CARGO_TARGET_OUTPUT_DIR = $(CARGO_TARGET_DIR)\$(CFG)
!endif
!if "$(CFG)" == "release" || "$(CFG)" == "Release"
CARGO_CMD = $(CARGO_CMD) --release
!endif

# For building the Rust bits for ARM64 Windows, or when we are building on
# ARM64 or 32-bit Windows for x64, we need to use a cross compiler,
# and it requires us to set up a default cmd.exe environment without any of the
# MSVC envvars, except the absolutely necessary ones.  So, we need to put those
# and the calls to cargo and therefore rustc in a temporary .bat file and use
# 'start /i ...' to call that .bat file
!if $(FORCE_CROSS) > 0
build-$(PLAT)-$(CFG).pre.bat:
	@echo @echo off>$@
	@echo set CommandPromptType=>>$@
	@echo set DevEnvDir=>>$@
	@echo set MAKEDIR=>>$@
	@echo set MAKEFLAGS=>>$@
	@echo set Platform=>>$@
	@echo set VCIDEInstallDir=>>$@
	@echo set VCINSTALLDIR=>>$@
	@echo set VCToolsInstallDir=>>$@
	@echo set VCToolsRedistDir=>>$@
	@echo set VCToolsVersion=>>$@
	@echo set VisualStudioVersion=>>$@
	@echo set VS140COMNTOOLS=>>$@
	@echo set VS150COMNTOOLS=>>$@
	@echo set VS160COMNTOOLS=>>$@
	@echo set VSCMD_ARG_app_plat=>>$@
	@echo set VSCMD_VER=>>$@
	@echo set VSINSTALLDIR=>>$@
	@echo set WindowsLibPath=>>$@
	@echo set WindowsSdkBinPath=>>$@
	@echo set WindowsSdkDir=>>$@
	@echo set WindowsSDKLibVersion=>>$@
	@echo set WindowsSdkVerBinPath=>>$@
	@echo set WindowsSDKVersion=>>$@
	@echo set Windows_ExecutablePath_x86=>>$@
	@echo set Windows_ExecutablePath_x64=>>$@
	@echo set _NMAKE_VER=>>$@
	@echo set __DOTNET_ADD_64BIT=>>$@
	@echo set __DOTNET_ADD_32BIT=>>$@
	@echo set __DOTNET_PREFERRED_BITNESS=>>$@
	@echo set __VSCMD_PREINIT_VCToolsVersion=>>$@
	@echo set __VSCMD_PREINIT_VS160COMNTOOLS=>>$@
	@echo set __VSCMD_script_err_count=>>$@
	@echo if not "$(__VSCMD_PREINIT_PATH)" == "" set PATH=$(__VSCMD_PREINIT_PATH);%HOMEPATH%\.cargo\bin>>$@
	@echo if "$(__VSCMD_PREINIT_PATH)" == "" set PATH=c:\Windows\system;c:\Windows;c:\Windows\system32\wbem;%HOMEPATH%\.cargo\bin>>$@
	@echo set GTK_LIB_DIR=$(LIBDIR)>>$@
	@echo set SYSTEM_DEPS_LIBXML2_LIB=$(LIBXML2_LIB:.lib=)>>$@
	@if not "$(PKG_CONFIG_PATH)" == "" echo set PKG_CONFIG_PATH=$(PKG_CONFIG_PATH)>>$@
	@if not "$(PKG_CONFIG)" == "" echo set PKG_CONFIG=$(PKG_CONFIG)>>$@

build-$(PLAT)-$(CFG)-lib.bat: build-$(PLAT)-$(CFG).pre.bat
	@type $**>$@
	@echo $(CARGO_CMD) $(RUST_VERBOSE_FLAG) --package librsvg-c>>$@

build-$(PLAT)-$(CFG)-bin.bat: build-$(PLAT)-$(CFG).pre.bat
	@type $**>$@
	@echo $(CARGO_CMD) $(RUST_VERBOSE_FLAG) --bin rsvg-convert>>$@

$(RSVG_INTERNAL_LIB): build-$(PLAT)-$(CFG)-lib.bat
$(CARGO_TARGET_OUTPUT_DIR)\rsvg-convert.exe: build-$(PLAT)-$(CFG)-bin.bat

$(RSVG_INTERNAL_LIB)	\
$(CARGO_TARGET_OUTPUT_DIR)\rsvg-convert.exe:
	@echo Please do not manually close the command window that pops up...
	@echo.
	@echo If this fails due to LNK1112 or a linker executable cannot be found, run
	@echo 'nmake /f Makefile CFG=$(CFG) PREFIX=$(PREFIX) $**',
	@echo and then run 'start /i /wait cmd /c $**', and then continue
	@echo the build with your original NMake command line.
	@start "Building the Rust bits for $(PLAT) Windows MSVC Build, please do not close this console window..." /wait /i cmd /c $**

!else
$(RSVG_INTERNAL_LIB):
	@set PATH=%PATH%;%HOMEPATH%\.cargo\bin
	@set GTK_LIB_DIR=$(LIBDIR);$(LIB)
	@set SYSTEM_DEPS_LIBXML2_NO_PKG_CONFIG=1
	@set SYSTEM_DEPS_LIBXML2_LIB=$(LIBXML2_LIB:.lib=)
	@if not "$(PKG_CONFIG_PATH)" == "" set PKG_CONFIG_PATH=$(PKG_CONFIG_PATH)
	@if not "$(PKG_CONFIG)" == "" set PKG_CONFIG=$(PKG_CONFIG)
	$(CARGO_CMD) $(RUST_VERBOSE_FLAG) --package librsvg-c
	@set GTK_LIB_DIR=

$(CARGO_TARGET_OUTPUT_DIR)\rsvg-convert.exe:
	@set PATH=%PATH%;%HOMEPATH%\.cargo\bin
	@set GTK_LIB_DIR=$(LIBDIR);$(LIB)
	@set SYSTEM_DEPS_LIBXML2_NO_PKG_CONFIG=1
	@set SYSTEM_DEPS_LIBXML2_LIB=$(LIBXML2_LIB:.lib=)
	@if not "$(PKG_CONFIG_PATH)" == "" set PKG_CONFIG_PATH=$(PKG_CONFIG_PATH)
	@if not "$(PKG_CONFIG)" == "" set PKG_CONFIG=$(PKG_CONFIG)
	$(CARGO_CMD) $(RUST_VERBOSE_FLAG) --bin $(@B)
	@set GTK_LIB_DIR=
!endif

cargo-clean:
	@set PATH=%PATH%;%HOMEPATH%\.cargo\bin
	@if exist build-$(PLAT)-$(CFG).bat del /f/q build-$(PLAT)-$(CFG).bat
	@$(CARGO_CLEAN_CMD)
	
!else
!if "$(VALID_CFGSET)" == "FALSE"
!error You need to specify an appropriate config for your build, using CFG=Release|Debug
!endif
!endif
