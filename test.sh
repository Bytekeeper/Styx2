#!/bin/bash -e 
./build.sh
rm -rf ~/.scbw/games/GAME_TEST
#scbw.play --headless --bots 'Hao Pan' styx_z --game_name TEST
#scbw.play --headless --bots WillBot styx_z --game_name TEST
scbw.play --headless --bots 'Marine Hell' styx_z --map 'sscai/(2)HeartbreakRidge.scx' --game_name TEST --timeout_at_frame 30000 || true
# scbw.play --headless --bots 'Tomas Cere' styx_z --map 'sscai/(2)HeartbreakRidge.scx' --game_name TEST --timeout_at_frame 30000 || true
# scbw.play --headless --bots MadMixZ styx_z --game_name TEST
#scbw.play --headless --bots Ecgberht styx_z --game_name TEST
cp ~/.scbw/games/GAME_TEST/player_0.rep ~/cherryvis-docker/replays/
mkdir ~/cherryvis-docker/replays/player_0.rep.cvis || true
cp -r ~/.scbw/games/GAME_TEST/write_1/cvis/* ~/cherryvis-docker/replays/player_0.rep.cvis/
