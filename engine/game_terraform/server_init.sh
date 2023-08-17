#!/bin/bash

echo "RUST_BACKTRACE=\"1\"" >> /etc/environment

echo "Security measures"

sed -i 's/PasswordAuthentication yes/PasswordAuthentication no/g' /etc/ssh/sshd_config && service ssh restart

cat <<EOF > /etc/sysctl.d/1000-custom.conf

# Limit memory use to min, default, max bytes per buffer
net.ipv4.tcp_rmem = 4096 32768 32768
net.ipv4.tcp_wmem = 4096 65536 131072

# After no activity for X seconds, start sending Y keepalive probes, with Z seconds in between
net.ipv4.tcp_keepalive_time = 300
net.ipv4.tcp_keepalive_probes = 4
net.ipv4.tcp_keepalive_intvl = 30

# Get rid of orphans ASAP.
net.ipv4.tcp_max_orphans = 1024
net.ipv4.tcp_orphan_retries = 3
net.ipv4.tcp_max_tw_buckets = 1024
net.ipv4.tcp_fin_timeout = 16

# Limit SYN flood and RST spoofing
net.ipv4.tcp_max_syn_backlog = 64
net.ipv4.tcp_retries2 = 8
net.ipv4.tcp_syncookies = 1
net.ipv4.tcp_syn_retries = 3
net.ipv4.tcp_synack_retries = 2
net.ipv4.tcp_challenge_ack_limit = 500

# Optimization
net.ipv4.tcp_no_metrics_save = 1

# Enable Spoof protection (reverse-path filter)
# Turn on Source Address Verification in all interfaces to
# prevent some spoofing attacks
net.ipv4.conf.default.rp_filter=1
net.ipv4.conf.all.rp_filter=1

# Do not accept ICMP redirects (prevent MITM attacks)
net.ipv4.conf.all.accept_redirects = 0
net.ipv6.conf.all.accept_redirects = 0

# Do not send ICMP redirects (we are not a router)
net.ipv4.conf.all.send_redirects = 0

# Do not accept IP source route packets (we are not a router)
net.ipv4.conf.all.accept_source_route = 0
net.ipv6.conf.all.accept_source_route = 0
EOF

cat <<EOF > /etc/nftables.conf
#!/usr/sbin/nft -f

flush ruleset

# netdev runs very early but packets may be fragmented
table netdev filter {
	chain ingress {
		type filter hook ingress device eth0 priority -500;

		# drop IP fragments
		ip frag-off & 0x1fff != 0 counter # drop

		# TCP x-mas
		tcp flags & (fin|psh|urg) == fin|psh|urg counter drop

		# TCP null
		tcp flags & (fin|syn|rst|psh|ack|urg) == 0x0 counter drop

		# TCP MSS
		tcp flags syn tcp option maxseg size 1-535 counter # drop
	}
}

# mangle runs next
table inet mangle {
	chain prerouting {
		type filter hook prerouting priority -150;

		# Allow existing connections to continue, drop invalid packets
		ct state invalid counter drop

		# New TCP packets must be SYN
		tcp flags & (fin|syn|rst|ack) != syn ct state new counter drop
	}
}

table inet filter {
	# Garbage collected
	set ipv4_total {
		type ipv4_addr
		size 2048
		flags dynamic
	}

	# Expiry based
	set ipv4_new {
		type ipv4_addr
		size 2048
		flags dynamic, timeout
	}
	# Expiry based
	set ipv4_new_log {
		type ipv4_addr
		size 2048
		flags dynamic, timeout
	}

	# Expiry based
	set ipv4_established {
		type ipv4_addr
		size 2048
		flags dynamic, timeout
	}

	# Garbage collected
	set ipv6_total {
		type ipv6_addr;
		size 2048
		flags dynamic
	}

	# Expiry based
	set ipv6_new {
		type ipv6_addr;
		size 2048
		flags dynamic, timeout
	}

	# Expiry based
	set ipv6_new_log {
		type ipv6_addr;
		size 2048
		flags dynamic, timeout
	}

	# Expiry based
	set ipv6_established {
		type ipv6_addr;
		size 2048
		flags dynamic, timeout
	}

	chain inbound_ipv4 {
		# Limit connection rate per source IP (no log)
		ct state new add @ipv4_new { ip saddr timeout 30s limit rate over 3/second burst 15 packets } counter drop

		# Limit connection rate per source IP (log)
		ct state new add @ipv4_new_log { ip saddr timeout 30s limit rate over 3/second burst 12 packets } counter log prefix "IPv4 per-IP ratelimit: " drop

		# Limit connections per source IP
		ct state new add @ipv4_total { ip saddr ct count over 20 } counter log prefix "IPv4 per-IP limit: " reject

		# Limit packet rate per source IP
		ct state { established, related } add @ipv4_established { ip saddr timeout 30s limit rate over 2048/second burst 16384 packets } counter drop

		# Allow ICMP pings (with a global limit)
		icmp type echo-request limit rate 5/second accept
	}

	chain inbound_ipv6 {
		# Limit connection rate per source IP (no log)
		ct state new add @ipv6_new { ip6 saddr timeout 30s limit rate over 2/second burst 15 packets } counter drop

		# Limit connection rate per source IP (log)
		ct state new add @ipv6_new_log { ip6 saddr timeout 30s limit rate over 2/second burst 12 packets } counter log prefix "IPv6 per-IP ratelimit: " drop

		# Limit connections per source IP
		ct state new add @ipv6_total { ip6 saddr ct count over 6 } counter reject

		# Limit packet rate per source IP
		ct state { established, related } add @ipv6_established { ip6 saddr timeout 30s limit rate over 1024/second } counter drop

		# Neighbor discovery.
		icmpv6 type { nd-neighbor-solicit, nd-router-advert, nd-neighbor-advert } limit rate 10/second accept

		# Allow ICMP pings (with a global limit)
		icmpv6 type echo-request limit rate 5/second accept
	}

	chain inbound {
		# What follows this is a whitelist
		type filter hook input priority 0; policy drop;

		# Protocol-specific rules
		meta protocol vmap { ip : jump inbound_ipv4, ip6 : jump inbound_ipv6 }

		# Allow loopback
		iifname lo accept

		# Allow existing connections to continue, drop invalid packets
		ct state vmap { established : accept, related : accept, invalid : drop }

		# Allow SSH (with a global limit)
		tcp dport ssh ct count 32 accept

		# Allow HTTP (with a global limit)
		tcp dport { http, https } ct count 1500 accept
	}

	chain forward {
		# We are not a router.
		type filter hook forward priority 0; policy drop;
	}
}
EOF

echo "Updating"

apt update

echo "Uninstalling sysstat"

# sysstat is suspected to steal the CPU for ~10s every 30m
sudo apt -y purge sysstat

echo "Installing snap"

apt install -y snapd
snap install core;
snap refresh core;

echo "Installing linode token"

printf "dns_linode_key = $LINODE_TOKEN\ndns_linode_version = 4\n" > /root/linode.ini
chmod 600 /root/linode.ini

echo "Installing certbot"

snap install --classic certbot
ln -s /snap/bin/certbot /usr/bin/certbot
snap set certbot trust-plugin-with-root=ok
snap install certbot-dns-linode

printf "certbot certonly --agree-tos --non-interactive --dns-linode --dns-linode-credentials /root/linode.ini --dns-linode-propagation-seconds 180 --no-eff-email --no-redirect --email finnbearone@gmail.com -d $DOMAIN -d www.$DOMAIN -d $SERVER_ID.$DOMAIN" > get_ssl_cert.sh
chmod u+x /root/get_ssl_cert.sh
./get_ssl_cert.sh

echo "Installing service..."
cat <<EOF > /etc/systemd/system/game-server.service
[Unit]
Description=Game Server

[Service]
Type=simple
User=root
Group=root
Restart=always
RestartSec=3
EnvironmentFile=/etc/environment
WorkingDirectory=~
ExecStart=/root/server \
  --server-id $SERVER_ID \
  --ip-address $IP_ADDRESS \
  --domain $DOMAIN \
  --chat-log /root/chat.log \
  --trace-log /root/trace.log \
  --certificate-path /etc/letsencrypt/live/$DOMAIN/fullchain.pem \
  --private-key-path /etc/letsencrypt/live/$DOMAIN/privkey.pem

[Install]
WantedBy=multi-user.target
EOF

echo "Installing util scripts..."
printf "journalctl -a -f -o cat -u game-server" > /root/view-game-server-logs.sh
chmod u+x /root/view-game-server-logs.sh

printf "sudo systemctl restart game-server" > /root/restart-game-server.sh
chmod u+x /root/restart-game-server.sh

printf "journalctl -a --no-pager -o cat -u game-server | grep -i \$1" > /root/grep-game-server-logs.sh
chmod u+x /root/grep-game-server-logs.sh

printf "nohup watch -c -n 1 'top -b -n1 | head -n 10 | tee -a top.txt' &" > /root/top.sh
chmod u+x /root/top.sh

echo "Raising firewalls..."

sysctl --system
nft -f /etc/nftables.conf

echo "Enabling service..."
sudo systemctl daemon-reload
sudo systemctl start game-server
sudo systemctl enable game-server

echo "Init done."
