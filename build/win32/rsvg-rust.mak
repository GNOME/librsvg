!include detectenv-msvc.mak

!if "$(CARGO)" == ""
BUILD_RUST = 0
!else
BUILD_RUST = 1
!endif

!if "$(VALID_CFGSET)" == "TRUE"
BUILD_RUST = 1
!else
BUILD_RUST = 0
!endif

!if "$(BUILD_RUST)" == "1"
!if "$(CFG)" == "release" || "$(CFG)" == "Release"
CARGO_CMD = $(CARGO) build --release
!else
CARGO_CMD = $(CARGO) build
!endif

all: vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_internals\$(CFG)\rsvg_internals.lib

vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_internals\$(CFG)\rsvg_internals.lib:
	@set CARGO_TARGET_DIR=..\build\win32\vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_internals
	@set GTK_LIB_DIR=..\..\vs$(VSVER)\$(PLAT)\lib;$(LIB)
	@cd ..\..\rust
	$(CARGO_CMD) --verbose
	@cd ..\build\win32\vs$(VSVER)
	@set GTK_LIB_DIR=
	@set CARGO_TARGET_DIR=

clean:
	@set CARGO_TARGET_DIR=..\build\win32\vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_internals
	@cd ..\..\rust
	@$(CARGO) clean
	@cd ..\build\win32\vs$(VSVER)
	@set CARGO_TARGET_DIR=
	
!else
!if "$(VALID_CFGSET)" == "FALSE"
!error You need to specify an appropriate config for your build, using CFG=Release|Debug
!else
!error You need to specify an appropriate path for your cargo executable using CARGO=<path_to_cargo.exe>
!endif
!endif
