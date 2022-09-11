#pragma once
#define STDIN_FILENO 0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

extern int write(unsigned int fd, const char* buf, unsigned int count);