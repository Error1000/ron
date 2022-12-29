all: test-suite.s
	$(CC) -o emulator-testsuite test-suite.s $(LDFLAGS) $(LDLIBS)

install:
	rm -f ../../disk/emulator-testsuite
	cp emulator-testsuite ../../disk

clean:
	rm -f emulator-testsuite

