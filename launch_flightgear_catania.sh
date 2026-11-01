#!/usr/bin/env bash
# Lansează Scenariul Extrem Multi-Agent: CATANIA–FONTANAROSSA (LICC, Pista 26 — Sicilia)
#   - Apropiere deasupra Mării Ionice (Golfo di Catania) la X_rwy = -2450m (lat=37.471725, lon=15.107488)
#   - Deziniat sever față de pistă: Y_rwy = +165m lateral + Cap inițial 242° (-22° eroare față de RWY 26 = 263.97°)
#   - Ceață marină extremă CAT III (vizibilitate 240m, Daylight) + Atac activ GPS Spoofing (+250m)
#   - Preluare autonomă prin 5 Agenți AI în cascadă (Cyber-Sentinel + SWIR 2.2um + S-Turn Sea Intercept + Coastline Step + 2x2 Wing-Low Slip)
set -e

if [ "$(id -u)" -eq 0 ] && id monk >/dev/null 2>&1; then
  MONK_UID="$(id -u monk)"
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
export vblank_mode=0
export __GL_SYNC_TO_VBLANK=0

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="$SCRIPT_DIR/flightgear_catania_session.log"

FG_APP="/home/monk/flightgear-2024.1.7-linux-amd64.AppImage"
if [ ! -x "$FG_APP" ]; then
  FG_APP="/home/monk/Desktop/flightgear-2024.1.6-linux-amd64.AppImage"
fi
if [ ! -x "$FG_APP" ]; then
  FG_APP="/home/monk/flightgear-2024.1.6-linux-amd64.AppImage"
fi
chmod +x "$FG_APP" 2>/dev/null || true

if ! curl -fsS http://127.0.0.1:8080/api/health >/dev/null 2>&1; then
  if [ -x "$SCRIPT_DIR/target/release/aero-cockpit-server" ]; then
    (cd "$SCRIPT_DIR" && nohup ./target/release/aero-cockpit-server >/tmp/aero-cockpit-server.log 2>&1 &)
    sleep 0.6
  fi
fi

FG_ROOT_DIR="/home/monk/.fgfs/fgdata_2024_1"
for FG_PROTO_DIR in "$FG_ROOT_DIR/Protocol" "/home/monk/.fgfs/Protocol"; do
  if [ -d "$(dirname "$FG_PROTO_DIR")" ]; then
    mkdir -p "$FG_PROTO_DIR"
    cp -f "$SCRIPT_DIR/flightgear/aero_shield.xml" "$FG_PROTO_DIR/aero_shield.xml" 2>/dev/null || true
  fi
done
if [ -d "$FG_ROOT_DIR/Nasal" ]; then
  cp -f "$SCRIPT_DIR/flightgear/aero_shield_hud.nas" "$FG_ROOT_DIR/Nasal/aero_shield_hud.nas" 2>/dev/null || true
fi

echo "🌊✈️  Pornire Scenariu Extrem Multi-Agent: CATANIA–FONTANAROSSA (LICC RWY 26 — Marea Ionică)"
echo "   - Start deasupra mării: X_rwy = -2450m, Abatere laterală = +165m, Cap = 242° (-22° dezaliniat față de RWY 26 = 263.97°)"
echo "   - Ceață marină CAT III (240m) + Atac GPS Spoofing Activ (+250m) + Preluare Multi-Agent AI"

exec "$FG_APP" \
  "--fg-root=$FG_ROOT_DIR" \
  "--prop:/sim/rendering/vsync-enable=0" \
  "--prop:/sim/frame-rate-throttle-hz=60" \
  "--enable-terrasync" \
  "--aircraft=c172p" \
  "--lat=37.471725" \
  "--lon=15.107488" \
  "--altitude=496" \
  "--heading=242.0" \
  "--vc=72" \
  "--glideslope=-3.0" \
  "--roll=0" \
  "--pitch=-2.0" \
  "--timeofday=noon" \
  "--prop:/local-weather/autostart=0" \
  "--prop:/sim/gui/dialogs/metar/mode/local-weather=0" \
  "--prop:/environment/weather-scenario=Manual input" \
  "--metar=LICC 011200Z 17008KT 0240 FG NSC 20/20 Q1016" \
  "--prop:/environment/rain-norm=0.0" \
  "--prop:/environment/snow-norm=0.0" \
  "--prop:/environment/aircraft-effects/fog-level=0.0" \
  "--prop:/environment/aircraft-effects/frost-level=0.0" \
  "--prop:/controls/anti-ice/window-heat=1" \
  "--prop:/aero-shield/autoland-local=1" \
  "--prop:/aero-shield/autoland-cmd=1" \
  "--prop:/aero-shield/spoof-jam-active=1" \
  "--prop:/aero-shield/shield-protection=1" \
  "--prop:/fdm/jsbsim/running=1" \
  "--prop:/fdm/jsbsim/propulsion/set-running=-1" \
  "--prop:/fdm/jsbsim/propulsion/engine[0]/set-running=1" \
  "--prop:/engines/active-engine/running=1" \
  "--prop:/engines/engine[0]/running=1" \
  "--prop:/controls/switches/magnetos=3" \
  "--prop:/controls/switches/master-bat=1" \
  "--prop:/controls/switches/master-alt=1" \
  "--prop:/controls/engines/current-engine/mixture=1.0" \
  "--prop:/controls/engines/engine[0]/mixture=1.0" \
  "--prop:/controls/engines/engine/use-primer=0" \
  "--prop:/controls/engines/engine[0]/use-primer=0" \
  "--prop:/engines/active-engine/oil-temperature-degf=180" \
  "--prop:/engines/engine[0]/oil-temperature-degf=180" \
  "--prop:/controls/engines/current-engine/throttle=0.60" \
  "--prop:/controls/engines/engine[0]/throttle=0.60" \
  "--prop:/controls/flight/flaps=0.33" \
  "--prop:/controls/flight/aileron-trim=0.016" \
  "--prop:/controls/flight/elevator-trim=-0.020" \
  "--prop:/controls/flight/rudder-trim=0.032" \
  "--prop:/controls/gear/brake-parking=0" \
  "--generic=socket,out,60,127.0.0.1,5505,udp,aero_shield" \
  "--generic=socket,in,20,127.0.0.1,5506,udp,aero_shield" \
  "--telnet=socket,bi,60,127.0.0.1,5501,tcp" \
  "--config=$SCRIPT_DIR/flightgear/aero_shield_config.xml" \
  "--disable-hud" \
  "$@" 2>&1 | tee "$LOG_FILE"
