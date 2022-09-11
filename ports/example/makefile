all: example.c
	$(CC) -Wall -o example example.c $(LDFLAGS) $(LDLIBS) $(CFLAGS)

install:
	rm -f ../../disk/example
	cp example ../../disk

clean:
	rm -f example
