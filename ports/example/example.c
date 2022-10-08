#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

void adhoc_print_number(unsigned int n){
	if(n == 0){ fwrite("0", 1, 1, stdout); return; }
	unsigned int rev;
	unsigned int len = 0;
	for(rev = 0; n != 0; n /= 10){ rev = rev*10 + n%10; len++; }
	for(char c = '?'; len-- != 0; rev /= 10){ c = (rev%10)+'0'; fwrite(&c, 1, 1, stdout); }
}

int main(int argc, char** argv) {
	if(argc != 1) return 420;
	int fd = open("/file.txt", O_RDWR | O_CREAT | O_APPEND);
	char* str_already_there = malloc(15);
	if(str_already_there == NULL){ puts("Malloc failed!"); return -1; }
	str_already_there[14] = '\0';
	read(fd, str_already_there, 14);

	puts("strcmp test: ");
	if(strcmp(str_already_there, "Hello, world!\n") == 0)
		puts("You've already run this program before :0");
	else
		puts("");

	puts("strstr test: ");
	adhoc_print_number((unsigned int)(strstr(str_already_there, "world")-str_already_there));
	puts("");

	puts("strcat test: ");
	char v[14];
	v[0] = '\0';
	strcat(v, str_already_there);
	puts(v);

	puts("write test: ");
	write(fd, "Hello, world!\n", 14);
	puts("Hello, world!");

	close(fd);
}
