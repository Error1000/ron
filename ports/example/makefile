all: test.c
	$(CC) -Wall -o test test.c $(LDFLAGS) $(LDLIBS) $(CFLAGS)

install:
	rm ../../disk/test
	cp test ../../disk

clean:
	rm test
