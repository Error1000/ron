all: test.c
	$(CC) -Wall -o test test.c $(LDFLAGS) $(LDLIBS) $(CFLAGS)

install:
	rm -f ../../disk/test
	cp test ../../disk

clean:
	rm -f test
