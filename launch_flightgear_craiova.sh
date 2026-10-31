#!/usr/bin/env bash
# Lansează FlightGear conectat VIZUAL, cu BLACKBOX LOGGING și STABILIZATOR SAS la AERO-SHIELD LRCV V2.0:
#   - IMPORTANT: Pista Craiova (LRCV) se află la 191m (626 ft MSL) deasupra nivelului mării!
#     În FlightGear, --altitude este în feet MSL, deci pentru ~725 ft AGL (~220m deasupra solului)
#     folosim --altitude=1350 (626 ft sol + 724 ft AGL = 1350 ft MSL) + --glideslope=-3.0!
#   - Scrie logul detaliat în: ./flightgear_session.log și ./flightgear_blackbox.csv
set -e

if [ "$(id -u)" -eq 0 ] && id monk >/dev/null 2>&1; then
  MONK_UID="$(id -u monk)"
  echo "🔄 Detectat terminal root — comutare automată pe sesiunea desktop 'monk' (UID $MONK_UID)..."
  exec sudo -u monk env \
    HOME="/home/monk" \
    USER="monk" \
    LOGNAME="monk" \
    DISPLAY="${DISPLAY:-:0}" \
    XAUTHORITY="${XAUTHORITY:-/home/monk/.Xauthority}" \
    XDG_RUNTIME_DIR="/run/user/${MONK_UID}" \
    DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/${MONK_UID}/bus" \
    PATH="$PATH" \
    "$0" "$@"
fi

export HOME="/home/monk"
export DISPLAY="${DISPLAY:-:0}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/1000}"
export DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-unix:path=/run/user/1000/bus}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="$SCRIPT_DIR/flightgear_session.log"

FG_APP="/home/monk/flightgear-2024.1.7-linux-amd64.AppImage"
if [ ! -x "$FG_APP" ]; then
  FG_APP="/home/monk/Desktop/flightgear-2024.1.6-linux-amd64.AppImage"
fi
if [ ! -x "$FG_APP" ]; then
  FG_APP="/home/monk/flightgear-2024.1.6-linux-amd64.AppImage"
fi

chmod +x "$FG_APP" 2>/dev/null || true

# 1. Pornim automat serverul Rust aero-cockpit-server dacă nu rulează deja pe :8080
if ! curl -fsS http://127.0.0.1:8080/api/health >/dev/null 2>&1; then
  if [ -x "$SCRIPT_DIR/target/release/aero-cockpit-server" ]; then
    echo "🚀 Pornire automată aero-cockpit-server în fundal (:8080 / UDP :5505)..."
    (cd "$SCRIPT_DIR" && nohup ./target/release/aero-cockpit-server >/tmp/aero-cockpit-server.log 2>&1 &)
    sleep 0.6
  fi
fi

# 2. Asigurăm sincronizarea protocolului bidirecțional în FG_ROOT local (~/.fgfs)
FG_ROOT_DIR="/home/monk/.fgfs/fgdata_2024_1"
for FG_PROTO_DIR in \
  "$FG_ROOT_DIR/Protocol" \
  "/home/monk/.fgfs/Protocol"; do
  if [ -d "$(dirname "$FG_PROTO_DIR")" ]; then
    mkdir -p "$FG_PROTO_DIR"
    cp -f "$SCRIPT_DIR/flightgear/aero_shield.xml" "$FG_PROTO_DIR/aero_shield.xml" 2>/dev/null || true
  fi
done

COMMON_BRIDGES=(
  "--fg-root=$FG_ROOT_DIR"
  "--prop:/local-weather/autostart=0"
  "--prop:/sim/gui/dialogs/metar/mode/local-weather=0"
  "--prop:/environment/weather-scenario=Manual input"
  "--metar=XXXX 011200Z 08902KT 0240 FG NSC 18/18 Q1018"
  "--prop:/environment/rain-norm=0.0"
  "--prop:/environment/snow-norm=0.0"
  "--prop:/environment/turbulence/magnitude-norm=0.0"
  "--prop:/environment/aircraft-effects/fog-level=0.0"
  "--prop:/environment/aircraft-effects/frost-level=0.0"
  "--prop:/controls/anti-ice/window-heat=1"
  "--prop:/aero-shield/autoland-local=1"
  "--prop:/aero-shield/autoland-cmd=1"
  "--prop:/fdm/jsbsim/running=1"
  "--prop:/controls/engines/engine/use-primer=0"
  "--prop:/controls/engines/engine[0]/use-primer=0"
  "--prop:/engines/active-engine/oil-temperature-degf=180"
  "--prop:/engines/engine[0]/oil-temperature-degf=180"
  "--prop:/controls/engines/current-engine/throttle=0.70"
  "--prop:/controls/engines/engine[0]/throttle=0.60"
  "--prop:/controls/engines/current-engine/throttle=0.60"
  "--prop:/controls/flight/elevator-trim=-0.020"
  "--prop:/controls/gear/brake-parking=0"
  "--generic=socket,out,60,127.0.0.1,5505,udp,aero_shield"
  "--generic=socket,in,20,127.0.0.1,5506,udp,aero_shield"
  "--telnet=socket,bi,60,127.0.0.1,5501,tcp"
  "--config=$SCRIPT_DIR/flightgear/aero_shield_config.xml"
  "--disable-hud"
)

echo "✈️  Pornire FlightGear <-> AERO-SHIELD (Altitudine 950 ft MSL = ~525 ft / 160m AGL pe panta de 3.0° la 1.4 NM de LRCV)"
echo "📊 Blackbox & FG-MCP Control active în:"
echo "   - $LOG_FILE (liniile [FG-BLACKBOX t=...])"
echo "   - $SCRIPT_DIR/flightgear_blackbox.csv (telemetrie 2 Hz salvată de serverul Rust)"
echo "   - FG-MCP Telnet Property Server: 127.0.0.1:5501 (flightgear/fg_mcp_ctl.py)"

if [ "${1:-}" = "--launcher" ]; then
  shift
  "$FG_APP" --launcher "${COMMON_BRIDGES[@]}" "$@" 2>&1 | tee "$LOG_FILE"
elif [ "${1:-}" = "--unaligned" ]; then
  shift
  # Scenariul 2 în FlightGear: 1.5 NM înainte de RWY 09, azimut 86° (~140m dezaliniat față de 88.93°), 980 ft MSL (~555 ft / 170m AGL!)
  "$FG_APP" \
    --airport=LRCV \
    --runway=09 \
    --offset-distance=1.5 \
    --offset-azimuth=86 \
    --altitude=980 \
    --vc=75 \
    --heading=85 \
    --visibility=240 \
    --wind=89@2 \
    --disable-real-weather-fetch \
    --timeofday=noon \
    "${COMMON_BRIDGES[@]}" \
    "$@" 2>&1 | tee "$LOG_FILE"
elif [ "${1:-}" = "--autoland" ]; then
  shift
  "$FG_APP" \
    --airport=LRCV \
    --runway=09 \
    --offset-distance=1.4 \
    --altitude=950 \
    --vc=75 \
    --heading=88.93 \
    --visibility=240 \
    --wind=89@2 \
    --disable-real-weather-fetch \
    --timeofday=noon \
    --prop:/aero-shield/autoland-local=1 \
    "${COMMON_BRIDGES[@]}" \
    "$@" 2>&1 | tee "$LOG_FILE"
else
  # Apropiere stabilă la 1.4 NM, 950 ft MSL (~525 ft / 160m AGL pe panta de 3.0° deasupra Craiovei!)
  "$FG_APP" \
    --airport=LRCV \
    --runway=09 \
    --offset-distance=1.4 \
    --altitude=950 \
    --vc=75 \
    --heading=88.93 \
    --visibility=240 \
    --wind=89@2 \
    --disable-real-weather-fetch \
    --timeofday=noon \
    "${COMMON_BRIDGES[@]}" \
    "$@" 2>&1 | tee "$LOG_FILE"
fi
