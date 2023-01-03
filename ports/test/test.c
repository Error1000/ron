#include <unistd.h>
#include <sys/wait.h>
#include <stdio.h>
#include <stdlib.h>

int main() {
	int p[2];
	pipe(p);
	int cpid = fork();
	if(cpid == 0){
		// We are the child
		printf("In child!\n");
		close(p[1]); // Close write end
		char buf[101];
		while(1){
			int res = read(p[0], buf, 100);
			if(res <= 0) break;
			buf[res] = 0;
			printf("Read: \'%s\', res: %d!\n", buf, res);
		}
	}else{
		// We are the parent
		printf("In parent!\n");
		close(p[0]); // Close read end
		write(p[1], "Hello, ", 7);
		write(p[1], "world!", 6);
		close(p[1]); // Close write end
		waitpid(cpid, NULL, 0);
	}
}
