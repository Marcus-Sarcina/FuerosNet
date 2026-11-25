#!/bin/sh
# open.sh <serial>: restart the app and open the streaming page from the home screen
S="$1"
find() { adb -s $S shell uiautomator dump /sdcard/u.xml >/dev/null 2>&1; adb -s $S shell cat /sdcard/u.xml | python3 -c "
import sys,re
x=sys.stdin.read()
m=re.search(r'content-desc=\"$1[^\"]*\"[^>]*?bounds=\"\[(\d+),(\d+)\]\[(\d+),(\d+)\]\"',x)
print((int(m.group(1))+int(m.group(3)))//2,(int(m.group(2))+int(m.group(4)))//2) if m else print('')
"; }
adb -s $S shell input keyevent KEYCODE_WAKEUP; adb -s $S shell wm dismiss-keyguard
adb -s $S shell am force-stop com.comptus.fueros
adb -s $S shell monkey -p com.comptus.fueros -c android.intent.category.LAUNCHER 1 >/dev/null 2>&1
sleep 10
xy=$(find "stream.internal")
if [ -z "$xy" ]; then
  sub=$(find "Malkovich Manor"); [ -n "$sub" ] || { echo "no subnet on screen"; exit 1; }
  adb -s $S shell input tap $sub; sleep 4
  xy=$(find "stream.internal")
fi
[ -n "$xy" ] || { echo "no launcher on screen"; exit 1; }
adb -s $S shell input tap $xy
echo "tapped $xy"
