SSH_KEY := ../.ssh/id_rsa
SSH_OPTS := -i $(SSH_KEY) -o "UserKnownHostsFile=/dev/null" -o "StrictHostKeyChecking=no"
SCP_OPTS := $(SSH_OPTS) -C
SERVER_1 := 172.105.90.101
SERVER_2 := 45.79.157.100
SERVER_3 := 139.162.149.225
SERVER_4 := 139.144.238.23

.PHONY: all debug_core target/release/server

all: debug

debug:
	RUST_BACKTRACE=1 cargo run -- --min-bots 256 --chat-log /tmp/kiomet.chat --debug-http info

release:
	cargo run --release -- --min-bots 256 --debug-http info

target/release/server:
	#RUSTFLAGS="-Ctarget-feature=-retpoline,+mmx,+aes,+sse,+sse2,+sse3,+sse4.1,+sse4.2,+popcnt" cargo build --release
	cargo build --release

ssh1:
	ssh $(SSH_OPTS) root@$(SERVER_1)

ssh2:
	ssh $(SSH_OPTS) root@$(SERVER_2)

ssh4:
	ssh $(SSH_OPTS) root@$(SERVER_4)

server1: target/release/server
	scp $(SCP_OPTS) target/release/server root@$(SERVER_1):/root/server1
	ssh $(SSH_OPTS) root@$(SERVER_1) 'mv /root/server1 /root/server && ./restart-game-server.sh'

server2: target/release/server
	scp $(SCP_OPTS) target/release/server root@$(SERVER_2):/root/server1
	ssh $(SSH_OPTS) root@$(SERVER_2) 'mv /root/server1 /root/server && ./restart-game-server.sh'

server3: target/release/server
	scp $(SCP_OPTS) target/release/server root@$(SERVER_3):/root/server1
	ssh $(SSH_OPTS) root@$(SERVER_3) 'mv /root/server1 /root/server && ./restart-game-server.sh'

server4: target/release/server
	scp $(SCP_OPTS) target/release/server root@$(SERVER_4):/root/server1
	ssh $(SSH_OPTS) root@$(SERVER_4) 'mv /root/server1 /root/server && ./restart-game-server.sh'

upload_game_client_server1:
	cargo run --manifest-path ../engine/upload_game_client/Cargo.toml -- --path ../client/dist --url https://1.kiomet.com/admin/

upload_game_client_server2:
	cargo run --manifest-path ../engine/upload_game_client/Cargo.toml -- --path ../client/dist --url https://2.kiomet.com/admin/

upload_game_client_server3:
	cargo run --manifest-path ../engine/upload_game_client/Cargo.toml -- --path ../client/dist --url https://3.kiomet.com/admin/
