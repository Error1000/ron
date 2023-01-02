all: dash.c
	$(CC)  -Wall -o dash  dash.c $(LDFLAGS) $(LDLIBS)

clean:
	rm -f dash

install:
	rm -f ../../disk/dash
	cp dash ../../disk
