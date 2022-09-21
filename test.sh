#!/bin/bash -e 
./build.sh
rm -rf ~/.scbw/games/GAME_TEST
MAP='aiide/(4)Fortress.scx'
# MAP='aiide/(4)Python.scx'
BOT='Microwave'
# BOT='Stardust'
GAME_NAME="$BOT-$(expr \"$MAP\" : '.*)\([A-Za-z]*\).scx')"
REPLAY_NAME="$GAME_NAME.rep"

rm -rf ~/.scbw/games/GAME_"$GAME_NAME"
scbw.play --headless --bots "$BOT" styx_z --map "$MAP" --game_name "$GAME_NAME" --timeout_at_frame 30000 || true

cp ~/.scbw/games/GAME_"$GAME_NAME"/player_0.rep ~/cherryvis-docker/replays/"$REPLAY_NAME"
mkdir ~/cherryvis-docker/replays/"$REPLAY_NAME".cvis 2>/dev/null || true
cp -r ~/.scbw/games/"GAME_$GAME_NAME"/write_1/cvis/* ~/cherryvis-docker/replays/"$REPLAY_NAME".cvis/
