@echo on
:: vcvarsall.bat sets various env vars like PATH, INCLUDE, LIB, LIBPATH for the
:: specified build architecture
call "C:\Program Files (x86)\Microsoft Visual Studio\2017\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" x64
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
@set LIBXML2_VER=2.10.4
@set FREETYPE2_VER=2.13.0
@set PKG_CONFIG_VER=0.29.2

pip3 install --upgrade --user meson~=0.64 || goto :error
git clone --depth 1 --no-tags https://gitlab.gnome.org/GNOME/gdk-pixbuf.git
git clone --depth 1 --no-tags https://gitlab.gnome.org/GNOME/pango.git

:: build and install GDK-Pixbuf (includes glib, libpng, libjpeg-turbo and their deps)
md _build_gdk_pixbuf
cd _build_gdk_pixbuf
meson setup ../gdk-pixbuf --buildtype=release --prefix=%INST_PSX% -Dman=false
ninja install || goto :error
cd ..
rmdir /s/q _build_gdk_pixbuf
copy /b %INST%\lib\z.lib %INST%\lib\zlib.lib

:: Download rustup-init, pkg-config and FreeType and libxml2
:: (sadly there is no CUrl, but wget, so MSYS2 is needed temporarily)
:: %MSYS2_BINDIR% must be in PATH to find gzip/xz.
set PATH=%PATH%;%MSYS2_BINDIR%
%MSYS2_BINDIR%\wget https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe
%MSYS2_BINDIR%\wget https://pkgconfig.freedesktop.org/releases/pkg-config-%PKG_CONFIG_VER%.tar.gz
%MSYS2_BINDIR%\wget https://downloads.sourceforge.net/freetype/freetype-%FREETYPE2_VER%.tar.xz
%MSYS2_BINDIR%\wget https://download.gnome.org/sources/libxml2/2.10/libxml2-%LIBXML2_VER%.tar.xz

%MSYS2_BINDIR%\tar -xf pkg-config-%PKG_CONFIG_VER%.tar.gz
%MSYS2_BINDIR%\tar -Jxf freetype-%FREETYPE2_VER%.tar.xz
%MSYS2_BINDIR%\tar -Jxf libxml2-%LIBXML2_VER%.tar.xz
:: Having the gnutools/msys64 in the %PATH% during the MSVC builds
:: can cause trouble...
set PATH=%BASEPATH%
del /f/q pkg-config-%PKG_CONFIG_VER%.tar.gz freetype-%FREETYPE2_VER%.tar.xz libxml2-%LIBXML2_VER%.tar.xz

:: build and install pkg-config
set PATH=%BASEPATH%
cd pkg-config-%PKG_CONFIG_VER%
nmake /f Makefile.vc CFG=release || goto :error
copy /b release\x64\pkg-config.exe %INST%\bin
nmake /f Makefile.vc CFG=release clean
cd ..

:: build and install FreeType (needed at least until Cairo fixes its checks for FontConfig)
md _build_ft
cd _build_ft
meson setup ../freetype-%FREETYPE2_VER% --buildtype=release --prefix=%INST_PSX% --pkg-config-path=%INST%\lib\pkgconfig --cmake-prefix-path=%INST%
ninja install || goto :error
cd ..
rmdir /s/q _build_ft

::build and install libxml2 (use the fast NMake builds)
cd libxml2-%LIBXML2_VER%\win32
cscript configure.js zlib=yes iconv=no prefix=%INST%
nmake || goto :error
nmake install
nmake clean
cd ..\..

:: build and install Pango (with HarfBuzz and Cairo)
md _build_pango
cd _build_pango
meson setup ../pango --buildtype=release --prefix=%INST_PSX% -Dfontconfig=disabled --pkg-config-path=%INST%\lib\pkgconfig
:: ideally, we should use `ninja install || goto :error`, but let's allow this command to
:: fail due to a Cairo issue that FontConfig is insisted upon, see
:: https://gitlab.freedesktop.org/cairo/cairo/-/merge_requests/469
ninja install
cd ..
rmdir /s/q _build_pango

:: Install Rust
rustup-init -y --default-toolchain=stable-%RUST_HOST% --default-host=%RUST_HOST%

:: now build librsvg
cd win32
nmake /f generate-msvc.mak generate-nmake-files PYTHON=python || goto :error
nmake /f Makefile.vc CFG=release PREFIX=%INST% PKG_CONFIG=%INST%\bin\pkg-config.exe PKG_CONFIG_PATH=%INST%\lib\pkgconfig || goto :error

goto :EOF
:error
exit /b 1
