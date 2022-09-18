all: cat.c
	$(CC) -Wall -o cat cat.c $(LDFLAGS) $(LDLIBS) $(CFLAGS)

install:
	rm -f ../../disk/cat
	cp cat ../../disk

clean:
	rm -f cat
