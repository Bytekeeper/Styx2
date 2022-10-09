#!/bin/bash -e 
./build.sh
# MAP='aiide/(2)Benzene.scx'
MAP='aiide/(2)Destination.scx'
# MAP='aiide/(2)HeartbreakRidge.scx'
# MAP='aiide/(2)PolarisRhapsody.scx'
# MAP='aiide/(3)Aztec.scx'
# MAP='aiide/(3)Longinus2.scx'
# MAP='aiide/(3)TauCross.scx'
# MAP='aiide/(4)Andromeda.scx'
# MAP='aiide/(4)CircuitBreaker.scx'
# MAP='aiide/(4)EmpireoftheSun.scm'
# MAP='aiide/(4)FightingSpirit.scx'
# MAP='aiide/(4)Fortress.scx'
# MAP='aiide/(4)Python.scx'
# MAP='aiide/(4)Roadkill.scm'

# BOT='Pylon Puller'
# BOT='Steamhammer'
# BOT='Microwave'
# BOT='Bryan Weber'
# BOT='Stardust'
BOT='BananaBrain'
# BOT='PurpleWave'
# BOT='Dragon'
GAME_NAME="$(expr "$BOT" : '\([A-Za-z0-9]\{4\}\)')_$(expr "$MAP" : '.*)\([A-Za-z0-9]\{4\}\)')"
REPLAY_NAME="$GAME_NAME.rep"

rm -rf ~/.scbw/games/GAME_"$GAME_NAME"
scbw.play --headless --bots styx_z "$BOT" --map "$MAP" --game_name "$GAME_NAME" --timeout_at_frame 30000 || true

cp ~/.scbw/games/GAME_"$GAME_NAME"/player_0.rep ~/cherryvis-docker/replays/"$REPLAY_NAME"
mkdir ~/cherryvis-docker/replays/"$REPLAY_NAME".cvis 2>/dev/null || true
cp -r ~/.scbw/games/"GAME_$GAME_NAME"/write_0/cvis/* ~/cherryvis-docker/replays/"$REPLAY_NAME".cvis/
