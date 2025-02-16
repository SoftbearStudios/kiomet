GAME_ID = Kiomet

all: release

include ../makefiles/client.mk

manifest:
	cargo run --manifest-path ../engine/manifest/Cargo.toml -- \
		--name Kiomet \
		--domain kiomet.com \
		--description "Kiomet is a multiplayer real-time strategy game. It is free to play online in a web browser with no install required. Start with a small country and expand into unclaimed areas. Make strategic alliances and join forces to defeat rivals. Protect the king at all times."\
		--keywords tower \
		--background-color "#273644" \
		--theme-color "#273644" \
		--icons 512x512.png,512x340.png,1280x720.png,1920x1080.png \
		--screenshots ss1920x1080.png

