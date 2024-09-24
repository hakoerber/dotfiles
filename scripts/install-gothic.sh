#!/usr/bin/env bash

# Steam setting: Proton 4.11-13
#
# * It *must not* have any symlinks for the directmusic dlls like:
# pfx/dosdevices/c:/windows/syswow64/dmusic.dll

STEAMAPPS=$XDG_DATA_HOME/Steam/steamapps/

read -rp "Make sure that gothic was installed via Steam and started once! <Enter> to continue, <CTRL+C> to abort "

set -o nounset
set -o xtrace
set -o errexit

downloaddir="${XDG_DOWNLOAD_DIR}"/gothic

mkdir -p "${downloaddir}"
cd "${downloaddir}"

curl -C - -L -o gothic_patch_108k.exe "https://www.worldofgothic.de/download.php?id=15"
curl -C - -L -o gothic1_playerkit-1.08k.exe "https://www.worldofgothic.de/download.php?id=61"

curl -C - -L -o Definitive_Edition_2_2_8.exe "https://www.worldofgothic.de/download.php?id=1586"

# superseded by union
# curl -C - -L -O https://github.com/GothicFixTeam/GothicFix/releases/download/v1.8/G1Classic-SystemPack-1.8.exe

curl -C - -L -o LaaHack.zip https://www.worldofgothic.de/download.php?id=1457

curl -C - -L -O https://github.com/GothicFixTeam/GothicFix/releases/download/v1.8/Gothic1_PlayerKit-2.8.exe

curl -C - -L -o Ninja-2.9.14.exe "https://www.worldofgothic.de/download.php?id=1626"
curl -C - -L -o Union_1.0m_26.06.2022.exe "https://www.worldofgothic.de/download.php?id=1625"

curl -C - -L -o G1CP-1.2.0.exe "https://www.worldofgothic.de/download.php?id=1636"

curl -C - -L -o GD3D11-v17.8-dev9.zip https://github.com/kirides/GD3D11/releases/download/v17.8-dev9/GD3D11-v17.8-dev9.zip
curl -C - -L -o RiisisGothic1TextureMixV1.1.zip "https://www.worldofgothic.de/download.php?id=1458"

read -rp "During installation, use \"${STEAMAPPS}/common/Gothic\" as the install directory! <Enter> to continue, <CTRL+C> to abort "


export WINEPREFIX="${STEAMAPPS}/compatdata/65540/pfx/"

winetricks dotnet7
winetricks directmusic

wine "${downloaddir}"/gothic_patch_108k.exe
wine "${downloaddir}"/gothic1_playerkit-1.08k.exe

wine "${downloaddir}"/Gothic1_PlayerKit-2.8.exe
wine "${downloaddir}"/G1Classic-SystemPack-1.8.exe

unzip -u "${downloaddir}"/LaaHack.zip -d "${STEAMAPPS}/common/Gothic/system"

# wine "${downloaddir}"/Union_1.0m_26.06.2022.exe
wine "${downloaddir}"/Ninja-2.9.14.exe

wine "${downloaddir}"/G1CP-1.2.0.exe
wine "${downloaddir}"/Definitive_Edition_2_2_8.exe

unzip -u "${downloaddir}"/GD3D11-v17.8-dev9.zip -d "${STEAMAPPS}/common/Gothic/system"
unzip -u "${downloaddir}"/RiisisGothic1TextureMixV1.1.zip -d "${STEAMAPPS}/common/Gothic/Data"

read -rp "Now run the game once and exit! <Enter> to continue, <CTRL+C> to abort "

cd "${STEAMAPPS}"/common/Gothic

sed -i 's/^playLogoVideos=.*$/playLogoVideos=0\r/' system/Gothic.ini
sed -i 's/^sightValue=.*$/sightValue=14\r/' system/Gothic.ini
sed -i 's/^modelDetail=.*$/modelDetail=1\r/' system/Gothic.ini
sed -i 's/^subTitles=.*$/subTitles=1\r/' system/Gothic.ini
sed -i 's/^animatedWindows=.*$/animatedWindows=0\r/' system/Gothic.ini
sed -i 's/^bloodDetail=.*$/bloodDetail=2\r/' system/Gothic.ini
sed -i 's/^zVidResFullscreenX=.*$/zVidResFullscreenX=2560\r/' system/Gothic.ini
sed -i 's/^zVidResFullscreenY=.*$/zVidResFullscreenY=1440\r/' system/Gothic.ini
sed -i 's/^zDetailTexturesEnabled=.*$/zDetailTexturesEnabled=1\r/' system/Gothic.ini
sed -i 's/^zSubdivSurfacesEnabled=.*$/zSubdivSurfacesEnabled=1\r/' system/Gothic.ini

sed -i 's/^SimpleWindow=.*$/SimpleWindow=0\r/' system/SystemPack.ini
sed -i 's/^Gothic2_Control=.*$/Gothic2_Control=1\r/' system/SystemPack.ini
sed -i 's/^USInternationalKeyboardLayout=.*$/USInternationalKeyboardLayout=0\r/' system/SystemPack.ini
sed -i 's/^FPS_Limit=.*$/FPS_Limit=144\r/' system/SystemPack.ini
sed -i 's/^VerticalFOV=.*$/VerticalFOV=85\r/' system/SystemPack.ini

sed -i 's/^OutDoorPortalDistanceMultiplier=.*$/OutDoorPortalDistanceMultiplier=3\r/' system/SystemPack.ini
sed -i 's/^InDoorPortalDistanceMultiplier=.*$/InDoorPortalDistanceMultiplier=3\r/' system/SystemPack.ini
sed -i 's/^WoodPortalDistanceMultiplier=.*$/WoodPortalDistanceMultiplier=3\r/' system/SystemPack.ini
sed -i 's/^DrawDistanceMultiplier=.*$/DrawDistanceMultiplier=3\r/' system/SystemPack.ini

sed -i 's/^keyDelayRate=.*$/keyDelayRate=50\r/' system/SystemPack.ini
sed -i 's/^keyDelayFirst=.*$/keyDelayFirst=150\r/' system/SystemPack.ini

sed -i 's/^HideFocus=.*$/HideFocus=0\r/' system/SystemPack.ini

sed -i 's/^Scale=.*$/Scale=1.9\r/' system/SystemPack.ini
