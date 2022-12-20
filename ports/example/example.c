#include <fcntl.h>
#include <unistd.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

// Because i don't trust printf, because we are supposed to test it
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
	puts("Shows location of string \"world\" in string read from file!");
	puts("");
	puts("");

	puts("strcat test: ");
	char v[14];
	v[0] = '\0';
	strcat(v, str_already_there);
	puts(v);

	puts("strncpy test: ");
	strncpy(v, str_already_there, 14);
	puts(v);

	puts("strtok test: ");
	char* first_tok = strtok(v, " ");
	if(first_tok) puts(first_tok);
	puts("Splits string read from file and shows first token!");
	puts("");

	puts("putchar test: ");
	putchar('H');
	putchar('i');
	putchar('!');
	puts("");
	puts("");

	puts("fputs test: ");
	fputs("Hello, world!\n", stdout);
	puts("");

	puts("fgets test: ");
	fgets(v, 14, stdin);
	v[13] = '\0';
	fputs("fgets read: ", stdout);
	fputs(v, stdout);
	puts("");
	puts("");

	puts("scanf test: ");
	int scanf_n;
	scanf("%d", &scanf_n);
	fputs("Scanf read: ", stdout);
	adhoc_print_number(scanf_n);
	puts("");
	puts("");

	puts("write to file, test: ");
	write(fd, "Hello, world!\n", 14);
	puts("Hello, world!");

	printf("Printf test %%, \"%s\", %d, 0x%x!\n", "Helllooo, world!", 420690, 0xbeef);
	close(fd);
}
