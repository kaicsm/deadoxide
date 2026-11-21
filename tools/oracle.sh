#!/bin/sh
# Differential check of the direct port: run a Java probe on the ORIGINAL jar and `pz-dump` on the Rust port over the
# same install, and compare the two outputs byte for byte. Nothing game-derived is committed; only sha256 of the
# oracle output is kept in tests/golden/engine/<case>.sha256 so `cargo test` can check the port without Java.
#   sh tools/oracle.sh            all cases
#   sh tools/oracle.sh tiledefs   one case
cd "$(dirname "$0")/.." || exit 2
export PATH="$HOME/.cargo/bin:$PATH"
ROOT=$(cd .. && pwd)
HERE=$(pwd)
mkdir -p target/oracle target/original-probe tests/golden/engine
cargo build -q --release -p zomboid-engine || exit 1
fail=0
# JDK 26+ recognizes this option; JDK 25 rejects it. Preserve the opt-in
# where available without preventing the older runtime from starting probes.
java_with_final_field_mutation() {
  if java --enable-final-field-mutation=ALL-UNNAMED -version >/dev/null 2>&1; then
    java --enable-final-field-mutation=ALL-UNNAMED "$@"
  else
    java "$@"
  fi
}
# case <name> <ProbeClass> [probe args...]   (pz-dump gets: <name> <root> [args...])
run_case() {
  name=$1; probe=$2; shift 2
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  (cd "$ROOT" && java -cp "$HERE/target/original-probe:projectzomboid.jar" "$probe" "$@" >"$HERE/target/oracle/$name.java.txt" 2>"$HERE/target/oracle/$name.java.err") \
    || { tail -5 target/oracle/$name.java.err; echo "FAIL $name (java probe)"; fail=1; return; }
  ./target/release/pz-dump "$name" "$ROOT" "$@" >target/oracle/$name.rust.txt 2>target/oracle/$name.rust.err \
    || { tail -5 target/oracle/$name.rust.err; echo "FAIL $name (pz-dump)"; fail=1; return; }
  if cmp -s target/oracle/$name.java.txt target/oracle/$name.rust.txt; then
    shasum -a 256 target/oracle/$name.java.txt | cut -d' ' -f1 >tests/golden/engine/$name.sha256
    echo "OK   $name: $(wc -l <target/oracle/$name.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL $name: first differences:"; diff target/oracle/$name.java.txt target/oracle/$name.rust.txt | head -6; fail=1
  fi
}
# jre_case <name> <ProbeClass> [probe args...]: like run_case, on the game's own bundled JRE (the JDK library
# behaviour the game really has: UTF-8 decoding, Double.parseDouble), the probe compiled for it.
PZ_JRE=${PZ_JRE:-"$HOME/Library/Application Support/Steam/steamapps/common/ProjectZomboid/Project Zomboid.app/Contents/PlugIns/jre-$( [ "$(uname -m)" = arm64 ] && echo aarch64 || echo x86_64 )/Contents/Home/bin/java"}
jre_case() {
  name=$1; probe=$2; shift 2
  [ -x "$PZ_JRE" ] || { echo "FAIL $name: no game JRE at $PZ_JRE (set PZ_JRE)"; fail=1; return; }
  javac -nowarn --release 25 -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe-jre "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  (cd "$ROOT" && "$PZ_JRE" -cp "$HERE/target/original-probe-jre:projectzomboid.jar" "$probe" "$@" >"$HERE/target/oracle/$name.java.txt" 2>"$HERE/target/oracle/$name.java.err") \
    || { tail -5 target/oracle/$name.java.err; echo "FAIL $name (java probe)"; fail=1; return; }
  ./target/release/pz-dump "$name" "$ROOT" "$@" >target/oracle/$name.rust.txt 2>target/oracle/$name.rust.err \
    || { tail -5 target/oracle/$name.rust.err; echo "FAIL $name (pz-dump)"; fail=1; return; }
  if cmp -s target/oracle/$name.java.txt target/oracle/$name.rust.txt; then
    shasum -a 256 target/oracle/$name.java.txt | cut -d' ' -f1 >tests/golden/engine/$name.sha256
    echo "OK   $name: $(wc -l <target/oracle/$name.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL $name: first differences:"; diff target/oracle/$name.java.txt target/oracle/$name.rust.txt | head -6; fail=1
  fi
}
# server_case <name> <ProbeClass> [args...]: the probe runs INSIDE the fully booted original (ServerHarness.java): a
# headless dedicated server from the pinned jar, natives for this OS from $PZ_NATIVES, fresh scratch cache dir, fixed seed.
# Prefer the native libraries beside the player's game jar (Linux/GOG installs),
# while preserving the macOS bundle fallback and explicit environment overrides.
if [ -z "${PZ_NATIVES:-}" ]; then
  if [ -d "$ROOT/natives" ]; then
    PZ_NATIVES="$ROOT/natives"
  else
    PZ_NATIVES="$HOME/Library/Application Support/Steam/steamapps/common/ProjectZomboid/Project Zomboid.app/Contents/Java"
  fi
fi
server_case() {
  name=$1; probe=$2; shift 2
  [ -d "$PZ_NATIVES" ] || { echo "FAIL $name: no natives in $PZ_NATIVES (set PZ_NATIVES)"; fail=1; return; }
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -sourcepath tests/original_probe -d target/original-probe tests/original_probe/ServerHarness.java "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  (cd "$ROOT" && java_with_final_field_mutation -Djava.awt.headless=true -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" "$probe" "$HERE/target/oracle/pz-oracle-$name" "$@" >"$HERE/target/oracle/$name.java.txt" 2>"$HERE/target/oracle/$name.java.err") \
    || { echo "FAIL $name (server probe): Java stack trace and recent logs:"; tail -n 40 "target/oracle/$name.java.err"; fail=1; return; }
  ./target/release/pz-dump "$name" "$ROOT" "$@" >target/oracle/$name.rust.txt 2>target/oracle/$name.rust.err \
    || { tail -5 target/oracle/$name.rust.err; echo "FAIL $name (pz-dump)"; fail=1; return; }
  if cmp -s target/oracle/$name.java.txt target/oracle/$name.rust.txt; then
    shasum -a 256 target/oracle/$name.java.txt | cut -d' ' -f1 >tests/golden/engine/$name.sha256
    echo "OK   $name: $(wc -l <target/oracle/$name.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL $name: first differences:"; diff target/oracle/$name.java.txt target/oracle/$name.rust.txt | head -6; fail=1
  fi
}
# game_case <name> <ProbeClass> [args...]: the probe runs parts of the ORIGINAL game standalone (no server): the pinned
# jar with this OS's natives, from the game root; its first argument is a fresh scratch cache dir, then the args.
# A probe may use another probe's code (javac finds it in tests/original_probe).
game_case() {
  name=$1; probe=$2; shift 2
  [ -d "$PZ_NATIVES" ] || { echo "FAIL $name: no natives in $PZ_NATIVES (set PZ_NATIVES)"; fail=1; return; }
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -sourcepath tests/original_probe -d target/original-probe "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  scratch="$HERE/target/oracle/pz-oracle-$name"; rm -rf "$scratch"
  (cd "$ROOT" && java_with_final_field_mutation -Djava.awt.headless=true -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" "$probe" "$scratch" "$@" >"$HERE/target/oracle/$name.java.txt" 2>"$HERE/target/oracle/$name.java.err") \
    || { tail -5 target/oracle/$name.java.err; echo "FAIL $name (game probe)"; fail=1; return; }
  ./target/release/pz-dump "$name" "$ROOT" "$scratch" "$@" >target/oracle/$name.rust.txt 2>target/oracle/$name.rust.err \
    || { tail -5 target/oracle/$name.rust.err; echo "FAIL $name (pz-dump)"; fail=1; return; }
  if cmp -s target/oracle/$name.java.txt target/oracle/$name.rust.txt; then
    shasum -a 256 target/oracle/$name.java.txt | cut -d' ' -f1 >tests/golden/engine/$name.sha256
    echo "OK   $name: $(wc -l <target/oracle/$name.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL $name: first differences:"; diff target/oracle/$name.java.txt target/oracle/$name.rust.txt | head -6; fail=1
  fi
}
# game_measure <name> <ProbeClass> [args...]: a game probe whose output is input data for a generator (no Rust side).
game_measure() {
  name=$1; probe=$2; shift 2
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  scratch="$HERE/target/oracle/pz-oracle-$name"; rm -rf "$scratch"
  (cd "$ROOT" && java_with_final_field_mutation -Djava.awt.headless=true -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" "$probe" "$scratch" "$@" >"$HERE/target/oracle/$name.txt" 2>"$HERE/target/oracle/$name.err") \
    || { tail -5 target/oracle/$name.err; echo "FAIL $name (game probe)"; fail=1; return; }
  echo "MEASURED $name: target/oracle/$name.txt ($(wc -l <target/oracle/$name.txt | tr -d ' ') lines)"
}
# ai_case <name> <ProbeClass> [args...]: the probe calls the game's NATIVE assimp (libjassimp through jassimp.Jassimp). The
# x86-64 slice of the library is the reference (the arm64 slice contracts multiply-add and differs in the last bit of
# some rotation keys): on an arm64 Mac the x86-64 JRE of the game bundle runs the probe under Rosetta (PZ_JRE_X86).
PZ_JRE_X86=${PZ_JRE_X86:-"$HOME/Library/Application Support/Steam/steamapps/common/ProjectZomboid/Project Zomboid.app/Contents/PlugIns/jre-x86_64/Contents/Home/bin/java"}
ai_case() {
  name=$1; probe=$2; shift 2
  [ -d "$PZ_NATIVES" ] || { echo "FAIL $name: no natives in $PZ_NATIVES (set PZ_NATIVES)"; fail=1; return; }
  [ -x "$PZ_JRE_X86" ] || { echo "FAIL $name: no x86-64 JRE at $PZ_JRE_X86 (set PZ_JRE_X86)"; fail=1; return; }
  javac -nowarn --release 25 -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -sourcepath tests/original_probe -d target/original-probe-jre "tests/original_probe/$probe.java" 2>target/oracle/$name.javac \
    || { cat target/oracle/$name.javac; echo "FAIL $name (javac)"; fail=1; return; }
  if [ "$(uname -m)" = arm64 ]; then run_x86() { arch -x86_64 "$PZ_JRE_X86" "$@"; }; else run_x86() { "$PZ_JRE_X86" "$@"; }; fi
  (cd "$ROOT" && run_x86 --enable-native-access=ALL-UNNAMED -Djava.awt.headless=true -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe-jre:projectzomboid.jar" "$probe" "$@" >"$HERE/target/oracle/$name.java.txt" 2>"$HERE/target/oracle/$name.java.err") \
    || { tail -5 target/oracle/$name.java.err; echo "FAIL $name (assimp probe)"; fail=1; return; }
  ./target/release/pz-dump "$name" "$ROOT" "$@" >target/oracle/$name.rust.txt 2>target/oracle/$name.rust.err \
    || { tail -5 target/oracle/$name.rust.err; echo "FAIL $name (pz-dump)"; fail=1; return; }
  if cmp -s target/oracle/$name.java.txt target/oracle/$name.rust.txt; then
    shasum -a 256 target/oracle/$name.java.txt | cut -d' ' -f1 >tests/golden/engine/$name.sha256
    echo "OK   $name: $(wc -l <target/oracle/$name.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL $name: first differences:"; diff target/oracle/$name.java.txt target/oracle/$name.rust.txt | head -6; fail=1
  fi
}
want=${1:-all}
sel() { [ "$want" = all ] || [ "$want" = "$1" ]; }
sel tiledefs && run_case tiledefs TileDefsProbe
# every media/fonts/**/*.fnt (and media/*.fnt) through AngelCodeFont, and TextManager.Init per language and font size
sel fonts && game_case fonts FontsProbe
sel sandbox && run_case sandbox SandboxProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13/map_sand.bin"
sel lotheaders && run_case lotheaders LotHeadersProbe "Muldraugh, KY"
sel worlddictionary && run_case worlddictionary WorldDictionaryProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13/WorldDictionary.bin"
sel lotpack && run_case lotpack LotPackProbe "Muldraugh, KY" 30 43 34 47
# the whole map (4.2 GB of lotpacks, minutes): sh tools/oracle.sh lotpack-all
[ "$want" = lotpack-all ] && run_case lotpack LotPackProbe "Muldraugh, KY" 0 0 99 99
sel metagrid && server_case metagrid MetaGridProbe "Muldraugh, KY" 30 43 34 47
# not in the default run until it matches (first gap: one Rand draw for crafted_01_29, a water-collector entity)
sel chunkobjects && server_case chunkobjects ChunkObjectsProbe 1036 1460 1039 1463
# a whole cell (1,024 chunks): sh tools/oracle.sh chunkobjects-cell
[ "$want" = chunkobjects-cell ] && server_case chunkobjects ChunkObjectsProbe 1024 1440 1055 1471
sel collide && server_case collide CollideProbe 1036 1460 1039 1463
sel entitysprites && server_case entitysprites EntitySpritesProbe
sel specialcollide && server_case specialcollide SpecialCollideProbe 129 182
sel basements && server_case basements BasementsProbe 30 43 34 47
# every chunk of the isolated profile's save, loaded by the original server and by the port
sel chunksave && server_case chunksave ChunkSaveProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the save's basements (map_basements.bin, seeded into the server's save) and map_meta.bin, read by IsoMetaGrid.load
sel mapmeta && server_case mapmeta MapMetaProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the save's zones (map_zone.bin) read by IsoMetaGrid.loadZone into the booted world's meta grid
sel mapzone && server_case mapzone MapZoneProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the same plus the outline of every wide polyline zone (native ClipperOffset + the Clipper64 union, ported Clipper2)
sel zoneoutlines && server_case zoneoutlines MapZoneProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" outlines
# the same plus Zone.getTotalArea of every zone: the native Clipper union and earcut triangles of polygons and outlines
sel zonetriangles && server_case zonetriangles MapZoneProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" triangles
# the save's metagrid/metacell_*.bin (IsoMetaCell.load) and map_animals.bin (IsoMetaGrid.loadAnimalZones)
sel animalzones && server_case animalzones AnimalZonesProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the save's map_t.bin (GameTime.load with ClimateManager.load) as a single-player game reads it
sel gametime && server_case gametime GameTimeProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the original ClimateManager (with ClimateValues, ClimateForecaster, WeatherPeriod, ThunderStorm) stepped over simulated days
# inside the booted dedicated server; scenarios seed:year:month:day:hour:days[:options]: a summer week; spring storms with
# running thunder clouds; a winter blizzard; a tropical storm; then the sandbox options (Rain, ClimateCycle, FogCycle,
# DayNightCycle) and the game modes that change the weather
sel climate && server_case climate ClimateProbe 1984:1993:6:8:9.0:6 12:1995:2:10:6.0:4 8:1995:0:10:6.0:4 8:1995:5:5:6.0:5 \
  "5:1995:1:3:6.0:2:climate=6,fog=1" "5:1995:4:3:6.0:2:climate=3,fog=3,rain=5" "5:1995:4:3:6.0:2:climate=2,fog=2,rain=1" \
  "5:1995:5:3:6.0:2:climate=5" "5:1995:10:3:6.0:5:mode=Winter is Coming" "5:1995:5:3:6.0:3:mode=The Descending Fog" \
  "5:1995:7:3:6.0:2:mode=A Storm is Coming" "5:1995:5:3:6.0:1:daynight=2" "5:1995:5:3:6.0:1:daynight=3"
# the original erosion system (ErosionMain with ErosionWorld, the eight categories, ErosionObj*, ErosionIceQueen, ErosionSeason)
# run by the booted server over chunks of the real map: chunk load, LoadGridsquare/ChunkLoaded, then simulated days of 144
# EveryTenMinutes; scenarios x1,y1,x2,y2:year:month:day:days:dumps[:options] (chunk coordinates; month and day 0-based; dump days
# separated by "/"; options speed, edays, apo, temp = the sandbox ErosionSpeed, ErosionDays, TimeSinceApo, Temperature, and
# keep=1 = start from the erosion.ini the previous scenario wrote): the load of a small stretch of Rosewood; a winter into
# spring over the same; the flowerbeds' chunks and a town through spring into summer; default options over 400 days; every
# option; and a chain that reads the ini back
sel erosion && server_case erosion ErosionProbe \
  "1033,1457,1038,1462:1993:6:8:0:0:speed=1" \
  "1033,1457,1038,1462:1993:9:20:200:40/80/120/160/200:speed=1" \
  "1017,1418,1020,1421:1993:3:10:100:20/50/100:speed=1" \
  "1327,1285,1332,1290:1993:3:10:100:20/50/100:speed=1" \
  "1034,1458,1036,1460:1993:6:8:400:100/200/300/400" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:speed=2" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:speed=4" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:speed=5" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:edays=30" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:edays=-1" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:apo=7" \
  "1034,1458,1036,1460:1993:6:8:20:0/20:apo=13,speed=5" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:temp=1" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:temp=2" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:temp=4" \
  "1034,1458,1036,1460:1993:6:8:40:0/40:temp=5" \
  "1034,1458,1036,1460:1993:6:8:3:0/3:speed=1" \
  "1034,1458,1036,1460:1993:6:11:4:0/4:speed=5,keep=1" \
  "1034,1458,1036,1460:1994:0:1:2:2:speed=3,apo=5,temp=1,keep=1"
# the original zombie.characters.IsoZombie, constructed headless inside the booted server over a fixed lore preset (the
# probe sets every ZombieLore option itself, so the server's own preset does not matter): scenarios seed:count:ticks[:Key=value,..]
# (the global Rand = java.util.Random(seed) with its draw counter at 0, OutfitRNG seeded likewise, Key = a ZombieLore option's
# name). Per zombie: the draw counter, the descriptor (names from the game's own name lists and translations), health,
# direction, palette, tints, the lore stats and speed, walk type, blood. The speed, strength, cognition, memory, sight, hearing
# and crawl-under-vehicle options each in every branch. A scenario with ticks > 0 makes ONE zombie, puts it alone on the square
# (8280, 11670, 0) of the loaded chunks 1034..1035 x 1458..1459 and runs preupdate/update/postupdate that many times (no
# player, no sound, no other zombie): the first state entry, the per-tick draw, the timers and the idle wander events
sel zombie && server_case zombie ZombieProbe \
  "12345:40:0" "1:40:0:Speed=4,SprinterPercentage=50" "2:30:0:Speed=1" "3:30:0:Speed=3" \
  "4:30:0:Speed=4,SprinterPercentage=0,Strength=4,Cognition=4,DoorOpeningPercentage=50" \
  "5:30:0:Memory=5,Sight=4,Hearing=4,CrawlUnderVehicle=2" \
  "6:30:0:Memory=6,Sight=5,Hearing=5,CrawlUnderVehicle=7,Strength=1,Cognition=1" \
  "7:30:0:Strength=3,Memory=1,CrawlUnderVehicle=1,Speed=4,SprinterPercentage=100" \
  "8:30:0:Memory=3,CrawlUnderVehicle=3,Sight=1,Hearing=3" "9:30:0:Memory=4,CrawlUnderVehicle=4,Sight=3,Hearing=1" \
  "10:30:0:CrawlUnderVehicle=6,Speed=2,Cognition=2" \
  "12345:1:9000" "1:1:12000" "2:1:9000:Speed=1" "3:1:9000:Speed=3" "4:1:6000:Speed=4" "5:1:15000" "6:1:9000" "7:1:9000" "8:1:9000"
# zombie.characters.action.ActionContext: the real context of an action group on a scripted owner, a seeded random walk
# over the variables and events its conditions test (state, child states, previous state, peekNextState, callbacks,
# canTransitionToState); the clip table of the human skeleton (clips); the real AdvancedAnimator (AnimLayer, LiveAnimNode,
# node selection, transitions, blends, track events) of an animation set on a scripted character, a seeded random walk over
# the variables its conditions read and the states (anim:<set>:<seed>:<ticks>[:state,..]), and the real zombie ticked alone
# with its ActionContext and animator (zanim:<seed>:<ticks>); see ActionContextProbe
sel actioncontext && server_case actioncontext ActionContextProbe \
  "ctx:zombie:11:4000" "ctx:zombie:12:4000" "ctx:zombie-crawler:13:3000" "ctx:player:14:6000" "ctx:player:15:6000" \
  "ctx:player-vehicle:16:3000" "ctx:rabbit:17:2000" "clips" \
  "zanim:12345:1200" "zanim:7:600" \
  "anim:zombie:51:1500" "anim:zombie:52:1500:idle,lunge,staggerback,staggerback-knockeddown,getup,getup-fromonback,getup-fromonfront,getup-fromsitting,walktoward,attack,hitreaction" \
  "anim:zombie-crawler:61:1200" \
  "anim:player:41:1500:idle,aim,aim-strafe,run,sprint,strafe,movement,melee,ext" "anim:player:43:1500" \
  "anim:player:45:1500:idle,movement,run,sprint,strafe,melee,actions,aim,aim-strafe,ext,sneak" "anim:player-vehicle:62:1200"
# the ORIGINAL zombie AI run headless in single-player mode (ZombieAiProbe: GameServer.server false, one player stand-in,
# zai:<seed>:<ticks>:<dx>:<dy>:<force|none>): the idle/face-target/lunge/attack/path-find/walk-toward states per tick with
# the animation variables, the draws and the angles; the first 24 scenarios end before the deferred (animation root) movement
# starts, the rest walk and lunge by it (E65a: keyframe root motion, collision, separate) and `zsq` prints the collide, path
# and vision matrices of the loaded squares
sel zombieai && server_case zombieai ZombieAiProbe \
  "zai:12345:200:0.6:0:force" "zai:1:60:0.6:0:force" "zai:2:60:0.6:0:force" "zai:3:60:0.6:0:force" \
  "zai:4:60:0.6:0:force" "zai:1:9:0:0.6:force" "zai:2:9:-0.6:0:force" "zai:3:9:0:-0.6:force" "zai:4:9:0.6:0.6:force" \
  "zai:5:9:2.0:0:force" "zai:6:9:0:2.0:force" "zai:7:9:-2.0:0:force" "zai:8:9:0:-2.0:force" "zai:9:9:3.0:0:force" \
  "zai:5:60:-1.0:-1.0:force" "zai:6:60:-2.0:2.0:force" "zai:12345:60:5.0:5.0:force" "zai:12345:2000:3.0:0:none" \
  "zai:7:2000:3.0:0:none" "zai:2:2000:3.0:0:none" "zai:12:2200:3.0:0:none" "zai:3:1389:3.0:0:none" \
  "zai:1:2269:3.0:0:none" "zai:9:1774:3.0:0:none" "zsq" \
  "zai:12345:200:2.0:0:force" "zai:1:200:0:2.0:force" "zai:2:200:-2.0:0:force" "zai:3:200:0:-2.0:force" \
  "zai:4:300:3.0:3.0:force" "zai:5:300:-3.0:1.0:force" "zai:12345:400:5.0:5.0:force" \
  "zai:3:3000:3.0:0:none" "zai:1:3000:3.0:0:none" "zai:9:3000:3.0:0:none" \
  "zai:6:300:-0.7:0:force" "zai:7:300:0.45:0.2:force" "zai:10:400:1.0:1.0:force" "zai:13:400:4.0:-3.0:force" \
  "zai:10:300:0.3:0.3:force" "zai:2:3000:3.0:0:none"
# the ORIGINAL zombie.pathfind in the booted server over real chunks of Rosewood (PathProbe): LineClearCollideMain, the
# vanilla PolygonalMap2.lineClearCollide/canStandAt of seeded random segments (lcc:<cx1>:<cy1>:<cx2>:<cy2>:<n>:<seed>, chunk
# rectangles plus a one-chunk ring; floors 0..2, the four collision flags, short/long/anywhere targets, centred or not)
sel pathfind && server_case pathfind PathProbe \
  "lcc:1033:1457:1038:1462:4000:7" "lcc:1008:1448:1015:1455:4000:11"
# the pathfinder's own cache in JAVA mode (PathfindNative.useNativeCode = false): zombie.pathfind.Square bits/cost/sloped
# surface of every square of two chunk rectangles (SquareUpdateTask.getBits via ChunkUpdateTask), and the high-level data
# (HLChunkLevel regions with their edge masks, staircases, sloped surfaces) of every chunk level
sel pathgraph && server_case pathgraph PathProbe \
  "pgraph:1033:1457:1038:1462" "pgraph:1008:1448:1015:1455"
# seeded path requests answered by the ORIGINAL Java PolygonalMap2 (HLAStar over chunk regions and staircases, VGAStar over
# squares, cleanPath, smoothPath for players, fixPathZ): zombie, crawler and player movers, random points and walkable squares
# (:sq) up to 24 squares and 2 levels apart; every node as Float.toString plus its flags
sel pathsearch && server_case pathsearch PathProbe \
  "psearch:1033:1457:1038:1462:100:7:zombie" "psearch:1033:1457:1038:1462:100:8:player" \
  "psearch:1033:1457:1038:1462:150:21:zombie:sq" "psearch:1033:1457:1038:1462:150:22:player:sq" \
  "psearch:1033:1457:1038:1462:100:23:crawler:sq" "psearch:1008:1448:1015:1455:150:24:player:sq" \
  "psearch:1008:1448:1015:1455:150:25:zombie:sq"
# the ORIGINAL IsoZombie.Hit run headless in the same single-player harness (zai:<seed>:<ticks>:<dx>:<dy>:hit<N>[,<M>...]
# [@<weapon>[@<damage split>[@c]]]; `c` = critical hit): at those ticks the player stand-in hits the zombie with a bat, axe,
# hunting knife, lead pipe or bare hands (E65b): the hit's draws (the item creation, the body part, processHit, the blood),
# the damage, StaggerBackState after a plain hit within reach, and after a critical hit ZombieFallDownState,
# ZombieOnGroundState and ZombieGetUpState (the reanimate timer), from several directions, with repeated hits
sel zombiehit && server_case zombiehit ZombieAiProbe \
  "zai:11:150:2.0:0:hit20@Base.BaseballBat@1.0" "zai:11:150:2.0:0:hit20@Base.Axe@1.0" \
  "zai:11:150:2.0:0:hit20@Base.HuntingKnife@1.0" "zai:11:150:2.0:0:hit20@Base.LeadPipe@1.0" \
  "zai:11:150:2.0:0:hit20@Base.BareHands@1.0" "zai:11:150:0.5:0:hit20@Base.BaseballBat@1.0" \
  "zai:11:150:0.5:0:hit20@Base.Axe@1.0" "zai:11:150:0.5:0:hit20@Base.HuntingKnife@1.0" \
  "zai:11:150:0.5:0:hit20@Base.LeadPipe@1.0" "zai:11:150:0.5:0:hit20@Base.BareHands@1.0" \
  "zai:12:150:2.0:0:hit20@Base.BaseballBat@1.0" "zai:12:150:2.0:0:hit20@Base.Axe@1.0" \
  "zai:12:150:2.0:0:hit20@Base.HuntingKnife@1.0" "zai:12:150:2.0:0:hit20@Base.LeadPipe@1.0" \
  "zai:12:150:2.0:0:hit20@Base.BareHands@1.0" "zai:12:150:0.5:0:hit20@Base.BaseballBat@1.0" \
  "zai:12:150:0.5:0:hit20@Base.Axe@1.0" "zai:12:150:0.5:0:hit20@Base.HuntingKnife@1.0" \
  "zai:12:150:0.5:0:hit20@Base.LeadPipe@1.0" "zai:12:150:0.5:0:hit20@Base.BareHands@1.0" \
  "zai:21:200:2.0:0:hit20@Base.BaseballBat@1.0@c" "zai:21:200:2.0:0:hit20@Base.Axe@1.0@c" \
  "zai:21:200:2.0:0:hit20@Base.HuntingKnife@1.0@c" "zai:21:200:2.0:0:hit20@Base.LeadPipe@1.0@c" \
  "zai:21:200:2.0:0:hit20@Base.BareHands@1.0@c" "zai:31:400:3.0:0:hit20@Base.BaseballBat@1.0@c" \
  "zai:32:400:3.0:0:hit20@Base.Axe@1.0@c" "zai:33:400:3.0:0:hit20@Base.HuntingKnife@1.0@c" \
  "zai:34:400:3.0:0:hit20@Base.LeadPipe@1.0@c" "zai:35:400:3.0:0:hit20@Base.BareHands@1.0@c" \
  "zai:36:400:4.0:-3.0:hit30@Base.BaseballBat@1.0@c" "zai:41:200:0:2.0:hit20@Base.BaseballBat@1.0" \
  "zai:42:200:-2.0:0:hit20@Base.Axe@1.0" "zai:43:200:0:-2.0:hit20@Base.LeadPipe@1.0" \
  "zai:44:200:1.5:1.5:hit20@Base.HuntingKnife@1.0" "zai:45:200:-1.5:-1.5:hit20@Base.BareHands@1.0" \
  "zai:46:200:0:0.5:hit20@Base.BaseballBat@1.0@c" "zai:47:180:-0.5:0:hit20@Base.Axe@1.0@c" \
  "zai:48:200:3.0:3.0:hit25@Base.LeadPipe@1.0@c" "zai:51:250:2.0:0:hit20,60@Base.BaseballBat@1.0" \
  "zai:52:250:0.5:0:hit20,50,80,110,140@Base.LeadPipe@1.0" "zai:53:250:3.0:0:hit20,25,30,35,40@Base.BareHands@1.0" \
  "zai:54:250:2.0:0:hit20,60,100@Base.Axe@0.5" "zai:55:250:0.5:0:hit20,24@Base.HuntingKnife@0.5"
# the save's global_mod_data.bin (GlobalModData.load) and iTrack.bin (InstanceTracker.load, exported as text and CSV)
sel moddata && server_case moddata ModDataProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# the outfits of media/clothing/clothing.xml (OutfitManager) and PersistentOutfits with the save's z_outfits.bin
sel outfits && server_case outfits OutfitsProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# media/fileGuidTable.xml and every clothing item file through OutfitManager.getClothingItem
sel clothingitems && server_case clothingitems ClothingItemsProbe
sel outfitrandom && server_case outfitrandom OutfitRandomProbe
sel animsets && game_case animsets AnimSetsProbe
sel luacompile && jre_case luacompile LuaCompileProbe "$HERE/tests/lua_compile_cases.txt"
sel luavm && jre_case luavm LuaVmProbe "$HERE/tests/lua_vm_cases.txt"
sel luastring && jre_case luastring LuaVmProbe "$HERE/tests/lua_string_cases.txt"
sel luaos && jre_case luaos LuaVmProbe "$HERE/tests/lua_os_cases.txt"
sel javachars && jre_case javachars JavaCharsProbe
# zombie.core.Translator: Languages.init + loadFiles for every language, getText & co., the formatter and org.json
sel translator && game_case translator TranslatorProbe "$HERE/tests/translator_cases.txt"
sel fdlibm && jre_case fdlibm FdLibmProbe
# zombie.iso.weather.SimplexNoise 3D + zombie.iso.worldgen.WorldGenSimplexGenerator: seeds, offsets and selector
sel wgsimplex && run_case wgsimplex WorldGenSimplexProbe
# zombie.iso.worldgen.WorldGenReader + biomes: the registry the game's own Lua defines (loadBiomes' parent
# merge, the selections, priorities) and BiomeRegistry's picks over the stage-1 simplex, both Lua states
# loaded by LoadDirBase of shared and server
sel wgbiomes && game_case wgbiomes WorldGenBiomesProbe
# zombie.iso.worldgen.utils + enums + utils.triangulation: the coordinate records, Direction, CacheMap,
# AreaSquareVar and the Delaunay triangulation, pure computation (no game state)
sel wgutils && run_case wgutils WgUtilsProbe
# zombie.iso.worldgen.WorldGenReader's remaining loaders: loadStaticModules (with the inline loadBiome and
# loadPrefab), loadVeinsConfig, loadRoadConfig, loadAnimalsPath, loadBiomeMapConfig, and the zombie_voronoi
# entries, over the same Lua state as wgbiomes
sel wgconfigs && game_case wgconfigs WgConfigsProbe
# zombie.iso.worldgen.zombie: ZombieVoronoi.getVoronois over the game's zombie_voronoi Lua,
# evaluateCellNoise/evaluateCellCutoff, the cell cache, and HashUtil.hash2D
sel wgvoronoi && game_case wgvoronoi WgVoronoiProbe
# zombie.iso.worldgen.veins: Veins.get over the game's worldgen.veins config with WorldGenParams seeds, the
# getRandom chain, OreVein's draws and isValid
sel wgveins && game_case wgveins WgVeinsProbe
# zombie.iso.worldgen.roads: RoadGenerator over the game's worldgen.roads config inside the booted server,
# the real meta grid bounds, with sorted output (the Delaunay edge set and the HashSet orders behind it are
# identity-hash based and not reproducible between JVM runs)
sel wgroads && server_case wgroads WgRoadsProbe
# Direct WorldGenTile oracle: seeded feature selection and private getBiomeTile with
# synthetic biomes, pending 16x16 groups, error branches, and post-call RNG states.
sel wgtiles && game_case wgtiles WgTilesProbe
# WorldGenChunk constructor/public queries against the original dedicated server's state.
# Generated square object streams and blending are deliberately outside this oracle's scope.
sel wgchunk && server_case wgchunk WgChunkProbe
# BiomeMap automatic PNG loading: active-map order, band extraction, missing-entry caching,
# cache eviction, and refusal to fall through when the first existing PNG is corrupt.
sel wgbiomemap && server_case wgbiomemap WgBiomeMapProbe
# WorldGenChunk.genMapChunk: a 2x2 prepopulated chunk cache, real tile definitions and Lua
# biome entries, and a generated PNG indexed via the original game file system.
sel wgmapchunk && server_case wgmapchunk WgMapChunkProbe
# Blending.applyBlending: a loaded 3x3 server-map fixture; exact tree, floor and handshake stream.
sel wgblending && server_case wgblending WgBlendingProbe
# AttachmentsHandler: real sprite catalog, full/partial and missing-neighbour paths, floor overlays.
sel wgattachments && server_case wgattachments WgAttachmentsProbe
# WorldGenChunk.applyPrefab via generateChunks with a controlled 2x2 static-module schematic.
sel wgprefabs && server_case wgprefabs WgPrefabsProbe
# Real lotpack-backed LoadBrandNew (2x2 per request) + 3x3 loaded chunk update.
sel wgfullworld && server_case wgfullworld WgFullWorldProbe
# Native world v249 chunk bytes + IsoChunk.LoadFromDiskOrBuffer round trip.
sel wgsavereload && server_case wgsavereload WgSaveReloadProbe
# IsoChunkMap resident-window scheduling: one-step scroll, reverse, teleport, negative coords.
sel wgstreamwindow && server_case wgstreamwindow WgStreamWindowProbe
# WorldGen zone Rocks, polygon and boundary semantics, including negative coordinates.
sel wgspecialzones && server_case wgspecialzones WgSpecialZonesProbe
# WorldGenChunk.generateChunks: a controlled empty 2x2 cache (one pre-existing object) on the
# booted original server, compared against the port using the same Lua tables and tile definitions.
sel wggenerate && server_case wggenerate WgGenerateProbe
sel luaload && game_case luaload LuaLoadEnvProbe shared
sel luaserver && game_case luaserver LuaLoadEnvProbe shared server
sel luaclient && game_case luaclient LuaLoadEnvProbe shared client
sel luaboot && game_case luaboot LuaBootProbe shared client
sel luagame && game_case luagame LuaGameProbe shared client
# the original ItemPickerJava on the game's own distribution Lua (shared + server, the distribution merge events, then
# Parse and InitSandboxLootSettings): the parsed tables sorted by key
sel loot && game_case loot LootProbe $(cat "$HERE/tests/loot_scenarios.txt")
# measurement only (no Rust side): what the original LuaManager.init exposes, the input of tools/gen_lua_expose.py
[ "$want" = luaexpose ] && game_measure luaexpose LuaExposeProbe
sel doortoggle && { cp tests/golden/engine/specialcollide.sha256 target/oracle/sc.keep 2>/dev/null; server_case specialcollide SpecialCollideProbe 129 182 toggle; mv tests/golden/engine/specialcollide.sha256 tests/golden/engine/doortoggle.sha256 2>/dev/null; cp target/oracle/sc.keep tests/golden/engine/specialcollide.sha256 2>/dev/null; }
sel los && server_case los LosProbe 129 182
sel losdoors && { server_case los LosProbe 129 182 toggle; cp tests/golden/engine/los.sha256 tests/golden/engine/losdoors.sha256 2>/dev/null; server_case los LosProbe 129 182; }
sel stairs && server_case stairs StairsProbe 126 181
# mover: a character walked over two staircases; the Rust side is the scene crate's example, not pz-dump.
mover_case() {
  args="8118.5 11615.5 0 -0.1 45 8077.5 11612.5 -0.1 0 45"
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe tests/original_probe/ServerHarness.java tests/original_probe/MoverProbe.java 2>target/oracle/mover.javac || { echo "FAIL mover (javac)"; fail=1; return; }
  (cd "$ROOT" && java_with_final_field_mutation -Djava.awt.headless=true -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" MoverProbe "$HERE/target/oracle/pz-oracle-mover" 126 181 $args >"$HERE/target/oracle/mover.java.txt" 2>"$HERE/target/oracle/mover.java.err") || { echo "FAIL mover (server probe)"; fail=1; return; }
  cargo run -q --release -p zomboid-scene --example mover_walk -- "$ROOT" 31 45 $args >target/oracle/mover.rust.txt 2>target/oracle/mover.rust.err || { echo "FAIL mover (rust)"; fail=1; return; }
  if cmp -s target/oracle/mover.java.txt target/oracle/mover.rust.txt; then
    cp target/oracle/mover.java.txt tests/golden/mover_stairs.txt
    echo "OK   mover: $(wc -l <target/oracle/mover.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL mover: first differences:"; diff target/oracle/mover.java.txt target/oracle/mover.rust.txt | head -6; fail=1
  fi
}
sel mover && mover_case
# movercollide: objects pushed against the walls, doors and furniture of the spawn house (and up a staircase).
movercollide_case() {
  cell=$1; rc=$2; shift 2
  javac -nowarn -encoding UTF-8 -cp "$ROOT/projectzomboid.jar" -d target/original-probe tests/original_probe/ServerHarness.java tests/original_probe/MoverCollideProbe.java 2>target/oracle/mc.javac || { echo "FAIL movercollide (javac)"; fail=1; return; }
  (cd "$ROOT" && java_with_final_field_mutation -Djava.awt.headless=true -Djava.library.path="$PZ_NATIVES" -Xmx4g -cp "$HERE/target/original-probe:projectzomboid.jar" MoverCollideProbe "$HERE/target/oracle/pz-oracle-mc" $cell "$@" >"$HERE/target/oracle/mc.java.txt" 2>"$HERE/target/oracle/mc.java.err") || { echo "FAIL movercollide (server probe)"; fail=1; return; }
  cargo run -q --release -p zomboid-scene --example mover_collide -- "$ROOT" $rc "$@" >target/oracle/mc.rust.txt 2>target/oracle/mc.rust.err || { echo "FAIL movercollide (rust)"; fail=1; return; }
  if cmp -s target/oracle/mc.java.txt target/oracle/mc.rust.txt; then
    cp target/oracle/mc.java.txt "tests/golden/mover_collide_$(echo $rc | tr ' ' '_').txt"
    echo "OK   movercollide $cell: $(wc -l <target/oracle/mc.java.txt | tr -d ' ') lines identical"
  else
    echo "FAIL movercollide $cell: first differences:"; diff target/oracle/mc.java.txt target/oracle/mc.rust.txt | head -6; fail=1
  fi
}
sel movercollide && movercollide_case "129 182" "32 45" 8303.5 11689.5 0.0697 0.0070 80 8303.5 11689.5 0.0617 0.0331 80 8303.5 11689.5 0.0443 0.0542 80 8303.5 11689.5 0.0202 0.0670 80 8303.5 11689.5 -0.0070 0.0697 80 8303.5 11689.5 -0.0331 0.0617 80 8303.5 11689.5 -0.0542 0.0443 80 8303.5 11689.5 -0.0670 0.0202 80 8303.5 11689.5 -0.0697 -0.0070 80 8303.5 11689.5 -0.0617 -0.0331 80 8303.5 11689.5 -0.0443 -0.0542 80 8303.5 11689.5 -0.0202 -0.0670 80 8303.5 11689.5 0.0070 -0.0697 80 8303.5 11689.5 0.0331 -0.0617 80 8303.5 11689.5 0.0542 -0.0443 80 8303.5 11689.5 0.0670 -0.0202 80 8288.5 11702.5 -0.05 0.01 40 8290.3 11698.2 0.06 0.06 80
sel movercollide && movercollide_case "126 181" "31 45" 8118.5 11615.5 0.001 -0.08 70
sel items && { python3 tools/gen_item_params.py >/dev/null; server_case items ItemsProbe "$HERE/target/oracle/item_fields.txt"; }
# every craftRecipe script (ScriptManager craftRecipe buckets: CraftRecipe.Load, OnScriptsLoaded, the ported part of
# OnPostWorldDictionaryInit) as the booted server holds it
sel recipes && server_case recipes RecipesProbe
# every vehicle script (VehicleScript.Load with its templates, Loaded) and vehicle template as the booted server holds it
sel vehicles && server_case vehicles VehiclesProbe
# the XUI scripts (xuiConfig, xuiSkin, ...) as ScriptManager loaded them, and XuiManager.ParseScripts' skins, styles, vars
sel xui && server_case xui XuiProbe
# mod loading over crafted mod folders in the probe's cache dir: ChooseGameInfo (mod.info, availability), the mod folders
# and their 42/42.x/common versions, ActiveMods, loadMods' order and the file map the mods' files shadow
sel mods && game_case mods ModsProbe "$HERE/tests/mods_cases.txt"
# every sound script (GameSoundScript.Load) and the GameSounds registry as the booted server holds them, then crafted
# Load bodies and GameSounds calls (tests/sounds_cases.txt); the second argument is the Rust side's scratch folder for
# the sounds.ini those commands write (the probe writes into the server's own cache folder)
sel sounds && server_case sounds SoundsProbe "$HERE/tests/sounds_cases.txt" "$HERE/target/oracle/pz-oracle-sounds-rust"
# Global object systems (gos_*.bin): the player's five real files, a file the ORIGINAL writer makes and hand-made broken
# ones; the second argument is the folder the probe (re)writes those crafted files into (tests/gos_crafted, our own
# data, committed) for the Rust side to read
sel gos && server_case gos GosProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/gos_crafted"
# Small world-level files (map_worldgen.bin, map_symbols.bin, statistics.bin, metadata.bin): the real files, the ones the
# ORIGINAL writers make, hand-made broken ones and the writer file cut at every length (tests/savemeta_crafted)
sel savemeta && server_case savemeta SaveMetaProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/savemeta_crafted"
# map_visited.bin (WorldMapVisited): the real file, the original's mutators and writer, and hand-made files of the current
# and the legacy formats (tests/visited_cases.txt); args: the commands, then the save folder for the real file
sel visited && server_case visited VisitedProbe "$HERE/tests/visited_cases.txt" "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# isoregiondata/datachunk_*.bin and RegionHeader.bin (the saved part of IsoRegions): IsoRegionWorker.loadSaveMap on the real folder
# and on the scenario folders of tests/isoregion_crafted (the ORIGINAL writer's files and hand-made ones, written by the probe)
sel isoregion && server_case isoregion IsoRegionProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/isoregion_crafted"
# erosion.ini (ErosionConfig.readFile/writeFile): the real file and the hand-made ones of tests/erosionini_crafted
sel erosionini && server_case erosionini ErosionIniProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/erosionini_crafted"
# map_t.bin's GMTM header and map_ver.bin (IsoWorld.readWorldVersion, GameWindow.ReadString(DataInputStream)): the real files and the pairs the probe writes to tests/mapver_crafted
sel mapver && server_case mapver MapVerProbe "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" "$HERE/tests/mapver_crafted"
# The radio (ZomboidRadio, RadioScriptManager, RadioChannel/Script/BroadCast/Line, RadioData.xml, RADIO_SAVE.txt, RadioTranslationData): the commands of tests/radio_cases.txt on the original and on the port; crafted files are in tests/radio_crafted, the 4th argument is the player's own save (its RADIO_SAVE.txt is read as @real)
sel radio && server_case radio RadioProbe "$HERE/tests/radio_cases.txt" "$HERE/tests/radio_crafted" "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13"
# 3D models and animation clips (E59). The `.txt` models of media/models and the `.txt` animations of media/animsold through
# ModelLoader.loadTxt (pure Java) in several (static, reverse, skinnedTo) combinations: format, vertex floats, element
# indices, bones, matrices, clips, keyframes and the per-bone tables, section digests (full dumps: TxtModelsProbe full <file>)
sel txtmodels && run_case txtmodels TxtModelsProbe
# every .x of media/models_X and media/anims_X imported by the native assimp with NO post-processing step, as an aiScene
# (meshes, bones, nodes, animation channels): the port of XFileImporter (the game's own steps follow in xmodelscene/animscene)
sel xscene && ai_case xscene AiSceneProbe none x media/models_X media/anims_X
# the same files with the game's own flags (FileTask_LoadAnimation: MAKE_LEFT_HANDED, REMOVE_REDUNDANT_MATERIALS; FileTask_LoadMesh:
# FIND_INSTANCES, MAKE_LEFT_HANDED, LIMIT_BONE_WEIGHTS, TRIANGULATE, OPTIMIZE_MESHES, REMOVE_REDUNDANT_MATERIALS, JOIN_IDENTICAL_VERTICES)
sel animscene && ai_case animscene AiSceneProbe anim x media/anims_X
sel xmodelscene && ai_case xmodelscene AiSceneProbe mesh x media/models_X
# what the game's own Java does with those scenes (ProcessedAiScene.process: ImportedSkeleton, ImportedSkinnedMesh,
# ImportedStaticMesh): the models in Normal and StaticMesh mode, by mesh name, all meshes, and skinned to another skeleton;
# the animations as AnimationOnly, with the .fbx scale/rotation modifiers, and skinned to a skeleton. Skeleton (bone indices,
# hierarchy, bind/inverse/skin matrices), every clip's keyframes, vertex arrays, elements, bounds and transforms, section digests
sel xmodels && ai_case xmodels ProcessedSceneProbe Normal,StaticMesh,Normal:last,StaticMesh:all,Normal:skin=media/models_X/Skinned/Cow_Skeleton.x mesh x media/models_X
sel animclips && ai_case animclips ProcessedSceneProbe AnimationOnly,AnimationOnly:fbx,AnimationOnly:skin=media/models_X/Skinned/Cow_Skeleton.x anim x media/anims_X
# zombie.characters.BodyDamage and the stat updates of IsoGameCharacter on a headless IsoPlayer in the booted server
sel body && server_case body BodyProbe "$HERE/tests/body_cases.txt"
# zombie.inventory item classes and containers (E67): InventoryItemFactory.CreateItem and what the item and container classes report, in the booted server
sel inventory && server_case inventory InventoryProbe "$HERE/tests/inventory_cases.txt"
# the same item and container code over the bytes the real game wrote (E67): every container and item that the port's chunk, vehicle
# and player loaders find in the isolated profile's save (`pz-dump invcorpus` writes them as rawitem/rawcont cases; nothing of the save
# is committed), loaded by the original and by the port, their state printed, and saved again (equal to the game's bytes or not)
if sel invbytes; then
  if [ -f "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13/players.db" ] \
     && ./target/release/pz-dump invcorpus "$ROOT" "$HERE/profile/Saves/Sandbox/2026-11-07_22-34-13" >target/oracle/invbytes.cases 2>target/oracle/invbytes.corpus.err; then
    server_case invbytes InventoryProbe "$HERE/target/oracle/invbytes.cases"
  else
    echo "FAIL invbytes: no corpus (the isolated profile's save, or pz-dump invcorpus)"; fail=1
  fi
fi
# the same for every .fbx of media/models_X and media/textures (2032 files, binary and ASCII): the port of assimp's FBX importer
# (FBXConverter: meshes, the $AssimpFbx$ chain nodes, skins, materials, animations). fbxscene: the importer alone (+mat also prints every
# material property: key, semantic, index, type, data); fbxmodelscene/fbxanimscene: after the game's own flags (TriangulateProcess,
# JoinIdenticalVertices, LimitBoneWeights, OptimizeMeshes, RemoveRedundantMaterials, FindInstances, MakeLeftHanded)
sel fbxscene && ai_case fbxscene AiSceneProbe none+mat fbx media/models_X media/textures
sel fbxmodelscene && ai_case fbxmodelscene AiSceneProbe mesh fbx media/models_X media/textures
sel fbxanimscene && ai_case fbxanimscene AiSceneProbe anim fbx media/models_X media/textures
# ProcessedAiScene.process over those: the models with the .fbx scale/rotation modifiers (FileTask_LoadMesh.loadFBX) in Normal and
# StaticMesh mode, by mesh name, all meshes and skinned to another skeleton; the animations (FileTask_LoadAnimation.loadFBX)
sel fbxmodels && ai_case fbxmodels ProcessedSceneProbe Normal:fbx,StaticMesh:fbx,Normal:last+fbx,StaticMesh:all+fbx,Normal:skin=media/models_X/Skinned/Cow_Skeleton.x+fbx mesh fbx media/models_X media/textures
sel fbxanims && ai_case fbxanims ProcessedSceneProbe AnimationOnly:fbx,AnimationOnly:skin=media/models_X/Skinned/Cow_Skeleton.x+fbx anim fbx media/models_X media/textures
# FMOD sound banks (E64): the game's own FMODManager.init() headless (natives of this OS, game root as cwd) loads the four
# banks media/sound/banks/Desktop/{ZomboidSound.strings,ZomboidMusic,ZomboidSound,ZomboidSoundMP}.bank; the probe prints what the
# Studio API reports (FmodBanksProbe <mode>), pz-dump the same from a parse of the bank files, which are RE'd (no FMOD source)
sel fmodbanks && game_case fmodbanks FmodBanksProbe banks
# the string table (STDT of ZomboidSound.strings.bank): every event's path and GUID per bank in API list order, the VCAs, FMODManager's
# sorted event path list, its parameter names, and GetBus/GetVCA over candidate paths the Rust side writes from its own table
sel fmodstrings && { ./target/release/pz-dump fmodcandidates "$ROOT" >target/oracle/fmod_candidates.txt && game_case fmodstrings FmodBanksProbe strings "$HERE/target/oracle/fmod_candidates.txt"; }
# the event and parameter records: per event the sustain flag, length (ms) and parameters (name, id, flags) the API reports, the global
# parameter list and FMODManager's parameter table (globalIndex); the id of a parameter no event uses is UNKNOWN on both sides
sel fmodevents && game_case fmodevents FmodBanksProbe events
# the sample data: the FSB5 inside the SND chunk of each data bank: header numbers and names read by the probe, stream count, frequency, channels,
# length (PCM and ms) and format of every stream from FMOD core opening the same head (sparse file in the scratch dir); the Vorbis decoder is not ported
sel fmodsamples && game_case fmodsamples FmodBanksProbe samples
# the same for every .glb of media/models_X (59 files, glTF 2.0 binary): the port of assimp's glTF2 importer (the accessors with the
# vertex remapping table, skins, materials, animations in libc++ unordered_map order). glbscene: the importer alone (+mat also prints
# every material property); glbmodelscene/glbanimscene: after the flags of FileTask_LoadMesh.loadGLTF / FileTask_LoadAnimation.loadGLTF
sel glbscene && ai_case glbscene AiSceneProbe none+mat glb media/models_X
sel glbmodelscene && ai_case glbmodelscene AiSceneProbe mesh glb media/models_X
sel glbanimscene && ai_case glbanimscene AiSceneProbe anim glb media/models_X
# ProcessedAiScene.process over those: the models (scale 1.0, no rotation: FileTask_LoadMesh.loadGLTF) in Normal and StaticMesh mode,
# by mesh name, all meshes and skinned to another skeleton; the animations (FileTask_LoadAnimation.loadGLTF: the same scale 0.01 and
# -90 degree rotation as an .fbx) alone and skinned to a skeleton
sel glbmodels && ai_case glbmodels ProcessedSceneProbe Normal,StaticMesh,Normal:last,StaticMesh:all,Normal:skin=media/models_X/Skinned/Cow_Skeleton.x mesh glb media/models_X
sel glbanims && ai_case glbanims ProcessedSceneProbe AnimationOnly:fbx,AnimationOnly:skin=media/models_X/Skinned/Cow_Skeleton.x+fbx anim glb media/models_X
# zombie.scripting.ScriptParser (readBlock, stripComments, parseTokens) over crafted text
sel scriptparser && jre_case scriptparser ScriptParserProbe "$HERE/tests/script_parser_cases.txt"
exit $fail
