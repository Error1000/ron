#pragma once
#include "stddef.h"
#define STDIN_FILENO 0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

extern ssize_t write(int fd, const void* buf, size_t count);
extern ssize_t read(int fd, void* buf, size_t count);
extern int close(int fd);