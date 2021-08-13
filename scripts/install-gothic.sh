#!/usr/bin/env bash

# Steam setting: Proton 4.11-13
#
# * It *must not* have any symlinks for the directmusic dlls like:
# pfx/dosdevices/c:/windows/syswow64/dmusic.dll

STEAMAPPS=/var/games/steamapps/

read -p "Make sure that gothic was installed via Steam and started once! <Enter> to continue, <CTRL+C> to abort "

set -o nounset
set -o xtrace
set -o errexit

downloaddir=~/download/gothic

mkdir -p "${downloaddir}"
cd "${downloaddir}"

curl -C - -L -o gothic_patch_108k.exe "https://www.worldofgothic.de/download.php?id=15"
curl -C - -L -o gothic1_playerkit-1.08k.exe "https://www.worldofgothic.de/download.php?id=61"

curl -C - -L -o Definitive_Edition_1_4_5.exe "https://www.worldofgothic.de/download.php?id=1586"

# superseded by union
curl -C - -L -O https://github.com/GothicFixTeam/GothicFix/releases/download/v1.8/G1Classic-SystemPack-1.8.exe

# curl -C - -L -O https://github.com/GothicFixTeam/GothicFix/releases/download/v1.8/Gothic1_PlayerKit-2.8.exe

curl -C - -L -o Ninja-2.5.09.exe "https://www.worldofgothic.de/download.php?id=1626"
# curl -C - -L -o Union_1.0j_22.02.2021.exe "https://www.worldofgothic.de/download.php?id=1625"

curl -C - -L -o G1CP-1.0.0.exe "https://www.worldofgothic.de/download.php?id=1636"

#curl -C - -L -o Spine_1.29.0.exe "https://www.worldofgothic.de/download.php?id=1417"

curl -C - -L -o Gothic1-GD3D11-17.7-dev16.zip https://github.com/Kirides/GD3D11/releases/download/v17.7-dev16/Gothic1-GD3D11-17.7-dev16.zip
curl -C - -L -o RiisisGothic1TextureMixV1.1.zip "https://www.worldofgothic.de/download.php?id=1458"

read -p 'During installation, use "Z:\var\games\steamapps\common\Gothic\" as the install directory! <Enter> to continue, <CTRL+C> to abort '


export WINEPREFIX="${STEAMAPPS}/compatdata/65540/pfx/"

#winetricks dxvk
winetricks directmusic

#read -p "In winecfg, go to Libraries tab, in 'existing overrides' search for 'dsound', select it and press remove button "
#winecfg

# wine "${downloaddir}"/gothic_patch_108k.exe
# wine "${downloaddir}"/gothic1_playerkit-1.08k.exe

# wine "${downloaddir}"/Gothic1_PlayerKit-2.8.exe
wine "${downloaddir}"/G1Classic-SystemPack-1.8.exe

wine "${downloaddir}"/Ninja-2.5.09.exe

# cmd="${WINEPREFIX}/dosdevices/c:/windows/syswow64/cmd.exe"
# cmdtarget="$(readlink "${cmd}")"
# rm "${WINEPREFIX}/dosdevices/c:/windows/syswow64/cmd.exe"
# winetricks cmd # for union install
# wine "${downloaddir}"/Union_1.0j_22.02.2021.exe
# ln -sf "$cmdtarget" "$cmd"

wine "${downloaddir}"/G1CP-1.0.0.exe
wine "${downloaddir}"/Definitive_Edition_1_4_5.exe

unzip -u "${downloaddir}"/Gothic1-GD3D11-17.7-dev16.zip -d "${STEAMAPPS}/common/Gothic/system"
unzip -u "${downloaddir}"/RiisisGothic1TextureMixV1.1.zip -d "${STEAMAPPS}/common/Gothic/Data"


read -p "Now run the game once and exit! <Enter> to continue, <CTRL+C> to abort "

cd /var/games/steamapps/common/Gothic

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
