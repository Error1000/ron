#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char** argv) {
	if(argc != 1) return 420;
	int fd = open("/file.txt", O_RDWR | O_CREAT | O_APPEND);
	char* str_already_there = malloc(15);
	if(str_already_there == NULL){ puts("Malloc failed!"); return -1; }
	str_already_there[14] = '\0';
	read(fd, str_already_there, 14);
	if(strcmp(str_already_there, "Hello, world!\n") == 0)
		puts("You've already run this program before :0");
	puts("Hello, world!");
	write(fd, "Hello, world!\n", 14);
	close(fd);
}
