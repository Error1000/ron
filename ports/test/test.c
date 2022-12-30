#include <unistd.h>
#include <sys/wait.h>
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
//		wait(NULL);
		exit(0);
	}
}
