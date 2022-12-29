#include <unistd.h>
#include <stdio.h>
#include <stdlib.h>

int main() {
	int pid = fork();
	if(pid == 0){
		// We are the child
		printf("In child!\n");
		exit(1);
	}else{
		// We are the parent
		printf("In parent!\n");
		exit(0);
	}
}
