!include detectenv-msvc.mak

!if "$(CARGO)" == ""
CARGO = cargo
!endif

!if "$(RUSTUP)" == ""
RUSTUP = rustup
!endif

!if "$(PLAT)" == "x64"
RUST_TARGET = x86_64
!else
RUST_TARGET = i686
!endif

!if "$(VALID_CFGSET)" == "TRUE"
BUILD_RUST = 1
!else
BUILD_RUST = 0
!endif

!if "$(BUILD_RUST)" == "1"

CARGO_TARGET = --target $(RUST_TARGET)-pc-windows-msvc
DEFAULT_TARGET = stable-$(RUST_TARGET)-pc-windows-msvc
RUSTUP_CMD = $(RUSTUP) default $(DEFAULT_TARGET)

!if "$(CFG)" == "release" || "$(CFG)" == "Release"
CARGO_CMD = $(CARGO) build $(CARGO_TARGET) --release
!else
CARGO_CMD = $(CARGO) build $(CARGO_TARGET)
!endif

all: vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_internals\$(RUST_TARGET)-pc-windows-msvc\$(CFG)\rsvg_internals.lib

vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_internals\$(RUST_TARGET)-pc-windows-msvc\$(CFG)\rsvg_internals.lib:
	@set CARGO_TARGET_DIR=..\win32\vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_internals
	@set GTK_LIB_DIR=..\..\vs$(VSVER)\$(PLAT)\lib;$(LIB)
	$(RUSTUP_CMD)
	@cd ..\rsvg_internals
	$(CARGO_CMD) --verbose
	@cd ..\win32\vs$(VSVER)
	@set GTK_LIB_DIR=
	@set CARGO_TARGET_DIR=

clean:
	@set CARGO_TARGET_DIR=..\win32\vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_internals
	@cd ..\rsvg_internals
	@$(CARGO) clean
	@cd ..\win32\vs$(VSVER)
	@set CARGO_TARGET_DIR=
	
!else
!if "$(VALID_CFGSET)" == "FALSE"
!error You need to specify an appropriate config for your build, using CFG=Release|Debug
!endif
!endif
