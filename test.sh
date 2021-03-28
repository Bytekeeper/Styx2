#!/bin/bash -e 
./build.sh
rm -rf ~/.scbw/games/GAME_TEST
#scbw.play --headless --bots 'Hao Pan' styx_z --game_name TEST
scbw.play --headless --bots WillBot styx_z --game_name TEST
cp ~/.scbw/games/GAME_TEST/player_0.rep ~/cherryvis-docker/replays/
