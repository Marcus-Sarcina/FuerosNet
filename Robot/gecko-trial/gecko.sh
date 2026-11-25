#!/bin/sh
# gecko.sh <true|false>: restart the local package, run the trial app on GXV walled or not, summarise
SP=/tmp/claude-1000/-home-aspiringsavage-Documents-FuerosNet/6e3d74b5-21da-45a9-859a-1903a3d97049/scratchpad
S=RFCRC1JDGXV
holder() { ss -ltnp 2>/dev/null | grep ':7461 ' | grep -o 'pid=[0-9]*' | cut -d= -f2 | head -1; }
old=$(holder); [ -n "$old" ] && kill $old; sleep 2
[ -z "$(holder)" ] || { echo "the old package still holds 7461"; exit 1; }
cd /home/aspiringsavage/Documents/FuerosNet/crates
RHTN_TESTAPP_FACE=stream RHTN_TESTAPP_PROBE=${PROBE:-192.168.0.11:7462} nohup ./target/debug/rhtn-testapp --listen 0.0.0.0:7461 --resource $(printf 'ab%.0s' $(seq 32)) > $SP/probe/local.log 2>&1 &
sleep 6
[ -n "$(holder)" ] || { echo "the package did not start"; cat $SP/probe/local.log; exit 1; }
grep -c '"bound":true' $SP/probe/local.log
adb -s $S shell input keyevent KEYCODE_WAKEUP; adb -s $S shell wm dismiss-keyguard
adb -s $S shell am force-stop com.comptus.geckotrial
adb -s $S logcat -c
adb -s $S shell am start -n com.comptus.geckotrial/.Main --ez walled $1 --es upstream 192.168.0.11:7461 > /dev/null
sleep ${2:-80}
adb -s $S logcat -d -s geckotrial | tail -6
python3 $SP/probe/sum.py $SP/probe/local.log
