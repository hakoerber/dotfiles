#!/usr/bin/env bash

read -p "Make sure that gothic was installed via Steam! <Enter> to continue, <CTRL+C> to abort "

set -o nounset
set -o xtrace
set -o errexit

downloaddir=~/download/gothic

mkdir -p "${downloaddir}"
cd "${downloaddir}"

curl -L -o gothic_patch_108k.exe "https://www.worldofgothic.de/download.php?id=15"
curl -L -o gothic1_playerkit-1.08k.exe "https://www.worldofgothic.de/download.php?id=61"
curl -L -o Definitive_Edition_1_4_2.exe "https://www.worldofgothic.de/download.php?id=1586"
curl -L -O https://github.com/GothicFixTeam/GothicFix/releases/download/v1.8/G1Classic-SystemPack-1.8.exe
curl -L -O https://github.com/GothicFixTeam/GothicFix/releases/download/v1.8/Gothic1_PlayerKit-2.8.exe

read -p "During installation, use \"Z:\\var\\games\\steamapps\\common\\Gothic\\\" as the install directory! <Enter> to continue, <CTRL+C> to abort "

export WINEPREFIX=/var/games/steamapps/compatdata/65540/pfx/

wine "${downloaddir}"/gothic1_playerkit-1.08k.exe
wine "${downloaddir}"/Gothic1_PlayerKit-2.8.exe
wine "${downloaddir}"/G1Classic-SystemPack-1.8.exe
wine "${downloaddir}"/Definitive_Edition_1_4_2.exe

read -p "Now run the game once and exit! <Enter> to continue, <CTRL+C> to abort "

cd /var/games/steamapps/common/Gothic

sed -i 's/^playLogoVideos=.*$/playLogoVideos=0\r/' system/Gothic.ini
sed -i 's/^sightValue=.*$/sightValue=14\r/' system/Gothic.ini
sed -i 's/^modelDetail=.*$/modelDetail=1\r/' system/Gothic.ini
sed -i 's/^subTitles=.*$/subTitles=0\r/' system/Gothic.ini
sed -i 's/^bloodDetail=.*$/bloodDetail=3\r/' system/Gothic.ini
sed -i 's/^zVidResFullscreenX=.*$/zVidResFullscreenX=2560\r/' system/Gothic.ini
sed -i 's/^zVidResFullscreenY=.*$/zVidResFullscreenY=1440\r/' system/Gothic.ini

sed -i 's/^SimpleWindow=.*$/SimpleWindow=0\r/' system/SystemPack.ini
sed -i 's/^Gothic2_Control=.*$/Gothic2_Control=1\r/' system/SystemPack.ini
sed -i 's/^USInternationalKeyboardLayout=.*$/USInternationalKeyboardLayout=0\r/' system/SystemPack.ini
sed -i 's/^FPS_Limit=.*$/FPS_Limit=144\r/' system/SystemPack.ini
sed -i 's/^VerticalFOV=.*$/VerticalFOV=50.625\r/' system/SystemPack.ini

sed -i 's/^OutDoorPortalDistanceMultiplier=.*$/OutDoorPortalDistanceMultiplier=3\r/' system/SystemPack.ini
sed -i 's/^InDoorPortalDistanceMultiplier=.*$/InDoorPortalDistanceMultiplier=3\r/' system/SystemPack.ini
sed -i 's/^WoodPortalDistanceMultiplier=.*$/WoodPortalDistanceMultiplier=3\r/' system/SystemPack.ini
sed -i 's/^DrawDistanceMultiplier=.*$/DrawDistanceMultiplier=3\r/' system/SystemPack.ini

sed -i 's/^HideFocus=.*$/HideFocus=0\r/' system/SystemPack.ini

sed -i 's/^Scale=.*$/Scale=1.9\r/' system/SystemPack.ini
