CC = gcc
CFLAGS = -Wall -Wextra -I./include
LDFLAGS = -lSDL2 -lncurses -lpthread -lm -ldl

RUST_LIB_DIR = target/release
RUST_LIB = $(RUST_LIB_DIR)/libsoundcloud_streamer.a

TARGET = sc_player
SRC = examples/main.c

all: $(TARGET)

$(RUST_LIB):
	cargo build --release

$(TARGET): $(SRC) $(RUST_LIB)
	$(CC) $(CFLAGS) $(SRC) $(RUST_LIB) $(LDFLAGS) -o $(TARGET)

clean:
	cargo clean
	rm -f $(TARGET)

.PHONY: all clean