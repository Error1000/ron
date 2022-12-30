#pragma once
#include <sys/types.h>

#define WUNTRACED 0b1

extern pid_t waitpid(pid_t pid, int* wstatus, int options);

#define wait(wstatus) waitpid(-1, wstatus, 0)
#define WIFEXITED(wstatus) (((wstatus & 0b111100000000) >> 8) == 1)
#define WEXITSTATUS(wstatus) (wstatus &     0b11111111)