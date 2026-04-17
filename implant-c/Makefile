CC = musl-gcc
CFLAGS = -Wall -Os -fno-asynchronous-unwind-tables -fno-ident -fomit-frame-pointer -ffunction-sections -fdata-sections -flto
LDFLAGS = -static -s -Wl,--gc-sections -Wl,--as-needed -Wl,-z,norelro
TARGET = implant

$(TARGET): implant.c
	$(CC) $(CFLAGS) $(LDFLAGS) -o $@ $<
	strip --strip-all -R .comment -R .note.* $(TARGET)

clean:
	rm -f $(TARGET)

.PHONY: clean