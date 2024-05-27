#!/usr/bin/env bash

# Steam setting: Proton 4.11-13
#
# * It *must not* have any symlinks for the directmusic dlls like:
# pfx/dosdevices/c:/windows/syswow64/dmusic.dll

set -o nounset
set -o xtrace
set -o errexit


STEAMAPPS=/var/games/steamapps/

BASEDIR="${STEAMAPPS}/common/Gothic II"

export GAMEDATA=${BASEDIR}/data/
export ARCHIVE=${BASEDIR}/gothic2.data.tar.zstd
export WINEPREFIX="${STEAMAPPS}/compatdata/39510/pfx/"
export WINEARCH=win64

export WINEVERSION=6.3

DOWNLOADDIR="${XDG_DOWNLOAD_DIR}"/gothic2

mkdir -p "${WINEPREFIX}"

# if [[ "$(wine --version)" != "wine-${WINEVERSION}" ]] ; then
#     printf '%s\n' "Wine version ${WINEVERSION} required" >&2
#     exit 1
# fi

archive() {
    origin="$1"
    if [[ -e "${ARCHIVE}" ]] ; then
        return
    fi
    tar -cv --zstd -p -f "${ARCHIVE}" -C "${GAMEDATA}" .
}

extract() {
    if [[ -e "${GAMEDATA}" ]] ; then
        return
    fi

    mkdir -p "${GAMEDATA}"
    tar x --zstd -f "${ARCHIVE}" -C "${GAMEDATA}"
}

start() {
    # For the DX11 renderer, PWD has to be the folder containing the "GD3D11" folder
    cd "${GAMEDATA}/system/"

    # There is a bug in the DX11 renderer that leads to the mouse cursor always
    # being visible
    #
    # See https://bugs.winehq.org/show_bug.cgi?id=48483
    #
    # Workaround is to disable the cursor completely during gameplay
    # if command -v unclutter >/dev/null ; then
    #     unclutter --timeout 0 --jitter 100000 --ignore-scrolling &
    #     pid=$!
    #     trap "kill $pid" EXIT
    # else
    #     echo "WARNING: Unclutter not installed, cannot disable mouse cursor"
    # fi

    wine ./Gothic2.exe
}

ini() {
    cd "${GAMEDATA}"

    set_ini() {
        local file="$1"
        local key="$2"
        local value="$3"
        if ! grep -q "^${key}=" "${file}" ; then
            echo "Key ${key} not fmund in ${file}"
            exit 1
        fi

        sed -i "s/^${key}=.*$/${key}=${value}\r/" "${file}"
    }

    set_ini system/Gothic.ini sightValue 14
    set_ini system/Gothic.ini modelDetail 1
    set_ini system/Gothic.ini animatedWindows 0
    set_ini system/Gothic.ini playLogoVideos 0
    set_ini system/Gothic.ini useGothic1Controls 1
    set_ini system/Gothic.ini keyDelayRate 50
    set_ini system/Gothic.ini keyDelayFirst 150
    set_ini system/Gothic.ini subTitles 1
    set_ini system/Gothic.ini invMaxColumns 8
    set_ini system/Gothic.ini invMaxRows 0
    set_ini system/Gothic.ini useQuickSaveKeys 1
    set_ini system/Gothic.ini zVidResFullscreenX 2560
    set_ini system/Gothic.ini zVidResFullscreenY 1440
    set_ini system/Gothic.ini zVidResFullscreenBPP 32
    set_ini system/Gothic.ini zRainWindScale 0.1
    set_ini system/Gothic.ini zMouseRotationScale 15.0
    set_ini system/Gothic.ini zSmoothMouse 0

    set_ini system/SystemPack.ini VerticalFOV 85.0
    set_ini system/SystemPack.ini DrawDistanceMultiplier 3
    set_ini system/SystemPack.ini OutDoorPortalDistanceMultiplier 3
    set_ini system/SystemPack.ini InDoorPortalDistanceMultiplier 3
}


case $1 in
    install)
        extract

        mkdir -p "${DOWNLOADDIR}"
        cd "${DOWNLOADDIR}"

        curl -C - -L -o g2addon-2_6.exe "https://www.worldofgothic.de/download.php?id=173"
        curl -C - -L -o gothic2_fix-2.6.0.0-rev2.exe "https://www.worldofgothic.de/download.php?id=833"
        curl -C - -L -o gothic2_playerkit-2.6f.exe "https://www.worldofgothic.de/download.php?id=518"
        curl -C - -L -o G2NoTR-SystemPack-1.8.exe "https://www.worldofgothic.de/download.php?id=1525"

        curl -C - -L -o LHiver204_DE_22-06-20.exe "https://www.worldofgothic.de/download.php?id=1580"

        curl -C - -L -o LaaHack.zip "https://www.worldofgothic.de/download.php?id=1457"

        curl -C - -L -O "https://github.com/Kirides/GD3D11/releases/download/v17.7-dev20/Gothic2-GD3D11-v17.7-dev20.zip"

        curl -C - -L -o Normalmaps_LHiver.zip "https://www.worldofgothic.de/download.php?id=1530"

        curl -C - -L -O https://github.com/Kirides/ninja-quickloot/releases/download/v1.9.5/Quickloot.vdf

        curl -C - -L -O https://github.com/szapp/Ninja/releases/download/v2.7.12/Ninja-2.7.12.exe

        # winetricks -q dxvk
        winetricks -q directmusic
        if command -v setup_dxvk >/dev/null ; then
            setup_dxvk install
        else
            echo "WARNING: Using dxvk via winetricks, untested"
            winetricks dxvk
        fi

        read -p 'During installation, use "${GAMEDATA//\//\\}" as the install directory! <Enter> to continue, <CTRL+C> to abort '

        wine "${DOWNLOADDIR}"/g2addon-2_6.exe
        wine "${DOWNLOADDIR}"/gothic2_fix-2.6.0.0-rev2.exe
        wine "${DOWNLOADDIR}"/gothic2_playerkit-2.6f.exe
        wine "${DOWNLOADDIR}"/G2NoTR-SystemPack-1.8.exe
        wine "${DOWNLOADDIR}"/LHiver204_DE_22-06-20.exe
        wine "${DOWNLOADDIR}"/Ninja-2.7.12.exe

        cd "${GAMEDATA}"

        unzip -o "${DOWNLOADDIR}"/Gothic2-GD3D11-v17.7-dev20.zip -d ./system/

        ln Data/ModVDF/LHE204_DE.mod Data/LHE204_DE.mod

        cp "${DOWNLOADDIR}"/Quickloot.vdf ./Data

        t="./system/GD3D11/Textures/replacements/Normalmaps_xxx"
        mkdir -p "${t}"
        set +o errexit
        unzip -o "${DOWNLOADDIR}"/Normalmaps_LHiver.zip -d "${t}"
        zip_exit="$?"
        set -o errexit
        if (( $zip_exit != 0 )) && (( $zip_exit != 2 )) ; then
            echo zip failed
            exit 1
        fi
        unset t

        laatmp=$(mktemp -d)

        unzip -o "${DOWNLOADDIR}"/LaaHack.zip -d "${laatmp}"

        read -p "For the LAA Hack, select only ${GAMEDATA}/system/Gothic2.exe [<Enter> to continue] "
        wine "${laatmp}"/LaaHack.exe
        rm -rf "${laatmp}"

        # Required to create all ini files
        read -p "Now run the game once and exit! [<Enter> to continue] "

        ini
        ;;
    ini)
        ini
        ;;
    start)
        export WINEDLLOVERRIDES="ddraw=n,b;dsound=b,n;d3dcompiler_47=n,b"
        start
        ;;
    archive)
        archive "$2"
        ;;
    exec)
        "${@}"
        ;;
esac
