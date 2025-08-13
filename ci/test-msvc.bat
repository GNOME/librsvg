@echo on
:: vcvarsall.bat sets various env vars like PATH, INCLUDE, LIB, LIBPATH for the
:: specified build architecture
call "C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64
@echo on

:: set PATH, LIB and INCLUDE to first include our install directory, as well as to where
:: `tar`, `bzip2` and `gzip` are.
@set INST=%CD%\rsvg.ci.bin
@set INST_PSX=%INST:\=/%
@set MSYS2_BINDIR=c:\msys64\usr\bin
@set BASEPATH=%INST%\bin;%PATH%
@set PATH=%BASEPATH%
@set LIB=%INST%\lib;%LIB%
@set INCLUDE=%INST%\include\glib-2.0;%INST%\lib\glib-2.0\include;%INST%\include;%INCLUDE%
@set RUST_HOST=x86_64-pc-windows-msvc

:: Packaged dep versions
@set LIBXML2_VER=2.12.6
@set FREETYPE2_VER=2.13.0
@set PKG_CONFIG_VER=0.29.2

@set CURRDIR=%CD%
pip3 install --upgrade --user -r ci/msvc-requirements.txt || goto :error
git clone --depth 1 --no-tags https://gitlab.gnome.org/GNOME/gdk-pixbuf.git
git clone --depth 1 --no-tags https://gitlab.gnome.org/GNOME/pango.git

:: build and install GDK-Pixbuf (includes glib, libpng, libjpeg-turbo and their deps)
md _build_gdk_pixbuf
cd _build_gdk_pixbuf
meson setup ../gdk-pixbuf --buildtype=release --prefix=%INST_PSX% -Dman=false -Ddocumentation=false
ninja install || goto :error
cd ..
rmdir /s/q _build_gdk_pixbuf
copy /b %INST%\lib\z.lib %INST%\lib\zlib.lib

:: Download rustup-init, pkg-config, FreeType and libxml2
:: (sadly there is no CUrl, but wget, so MSYS2 is needed temporarily)
:: %MSYS2_BINDIR% must be in PATH to find gzip/xz.
set PATH=%PATH%;%MSYS2_BINDIR%
if not exist %HOMEPATH%\.cargo\bin\rustup.exe %MSYS2_BINDIR%\wget https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe
%MSYS2_BINDIR%\wget https://pkgconfig.freedesktop.org/releases/pkg-config-%PKG_CONFIG_VER%.tar.gz
%MSYS2_BINDIR%\wget https://downloads.sourceforge.net/freetype/freetype-%FREETYPE2_VER%.tar.xz
%MSYS2_BINDIR%\wget https://download.gnome.org/sources/libxml2/2.12/libxml2-%LIBXML2_VER%.tar.xz
:: Ensure it sets the filename correctly
%MSYS2_BINDIR%\wget --content-disposition https://wrapdb.mesonbuild.com/v2/libxml2_%LIBXML2_VER%-1/get_patch

%MSYS2_BINDIR%\tar -xf pkg-config-%PKG_CONFIG_VER%.tar.gz
%MSYS2_BINDIR%\tar -Jxf freetype-%FREETYPE2_VER%.tar.xz
%MSYS2_BINDIR%\tar -Jxf libxml2-%LIBXML2_VER%.tar.xz
:: Sorry for this hack! Please remove when the runner gets unzip through Pacman
python -c "import zipfile; zipfile.ZipFile('libxml2_%LIBXML2_VER%-1_patch.zip', 'r').extractall()"
:: Having the gnutools/msys64 in the %PATH% during the MSVC builds
:: can cause trouble...
del /f/q pkg-config-%PKG_CONFIG_VER%.tar.gz freetype-%FREETYPE2_VER%.tar.xz libxml2-%LIBXML2_VER%.tar.xz libxml2_%LIBXML2_VER%-1_patch.zip 

:: build and install pkg-config
cd pkg-config-%PKG_CONFIG_VER%

:: patch pkg-config's NMake Makefile so that GNU's mkdir won'y be used by accident
%MSYS2_BINDIR%\patch -p1 < %CURRDIR:\=/%/ci/pkgconfig.nmake.patch
set PATH=%BASEPATH%
nmake /f Makefile.vc CFG=release || goto :error
copy /b release\x64\pkg-config.exe %INST%\bin
nmake /f Makefile.vc CFG=release clean
cd ..

:: build and install FreeType
md _build_ft
cd _build_ft
meson setup ../freetype-%FREETYPE2_VER% --buildtype=release --prefix=%INST_PSX% --pkg-config-path=%INST%\lib\pkgconfig --cmake-prefix-path=%INST%
ninja install || goto :error
cd ..
rmdir /s/q _build_ft

:: build and install libxml2 (use the Meson wrap overlaid before)
md _build_libxml
cd _build_libxml
meson setup ../libxml2-%LIBXML2_VER% --buildtype=release --prefix=%INST_PSX% -Diconv=disabled --pkg-config-path=%INST%\lib\pkgconfig --cmake-prefix-path=%INST%
ninja install || goto :error
cd ..
rmdir /s/q _build_libxml

:: build and install Pango (with HarfBuzz and Cairo)
md _build_pango
cd _build_pango
meson setup ../pango --buildtype=release --prefix=%INST_PSX% -Dfontconfig=disabled --pkg-config-path=%INST%\lib\pkgconfig
ninja install || goto :error
cd ..
rmdir /s/q _build_pango

:: Install Rust
if exist %HOMEPATH%\.cargo\bin\rustup.exe %HOMEPATH%\.cargo\bin\rustup update
if not exist %HOMEPATH%\.cargo\bin\rustup.exe rustup-init -y --default-toolchain=stable-%RUST_HOST% --default-host=%RUST_HOST%
%HOMEPATH%\.cargo\bin\cargo install cargo-c || goto :error

:: Enable workaround if latest stable Rust caused issues like #968.
:: Update RUST_DOWNGRADE_VER below as well as required.
@set DOWNGRADE_RUST_VERSION=0

:: now build librsvg
set PATH=%PATH%;%HOMEPATH%\.cargo\bin
set PKG_CONFIG=%INST%\bin\pkg-config.exe
md msvc-build
cd msvc-build

:: Fix linking to PCRE for CI's sake
if exist %INST%\lib\libpcre2-8.a copy /b %INST%\lib\libpcre2-8.a %INST%\lib\pcre2-8.lib

if not "%DOWNGRADE_RUST_VERSION%" == "1" goto :normal_rust_build
@set RUST_DOWNGRADE_VER=1.82.0
%HOMEPATH%\.cargo\bin\rustup install %RUST_DOWNGRADE_VER%-%RUST_HOST%
meson setup .. --buildtype=release --prefix=%INST_PSX% --pkg-config-path=%INST%\lib\pkgconfig --cmake-prefix-path=%INST% -Dtriplet=%RUST_HOST% -Drustc-version=%RUST_DOWNGRADE_VER% || goto :error
goto :continue_build

:normal_rust_build
meson setup .. --buildtype=release --prefix=%INST_PSX% --pkg-config-path=%INST%\lib\pkgconfig --cmake-prefix-path=%INST% || goto :error

:continue_build
ninja || goto :error
ninja test
ninja install || goto :error

goto :EOF
:error
exit /b 1
