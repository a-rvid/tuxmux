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
	$(CC) $(CFLAGS) $(LDFLAGS) -o $(TARGET) $<

$(TARGET): implant.c
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $<
	strip --strip-all -R .comment -R .note.* $(TARGET)

clean:
	rm -f $(TARGET)

.PHONY: clean dev