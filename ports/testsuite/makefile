all: test-suite.s
	$(CC) -o testsuite test-suite.s $(LDFLAGS) $(LDLIBS)

install:
	rm -f ../../disk/testsuite
	cp testsuite ../../disk

clean:
	rm -f testsuite

