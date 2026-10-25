CC ?= gcc
CFLAGS += -O2 -Wall
LDLIBS += -lX11 -lasound -lm -lrt
SRC = $(wildcard src/*.c)
bouncer: $(SRC)
	$(CC) $(CFLAGS) -o $@ $(SRC) $(LDLIBS) -Wl,--as-needed
