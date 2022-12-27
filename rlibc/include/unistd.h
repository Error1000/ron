#pragma once
#include "stddef.h"
#include "sys/types.h"

#define STDIN_FILENO 0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

#define SEEK_CUR 0
#define SEEK_SET 1
#define SEEK_END 3

extern ssize_t write(int fd, const void* buf, size_t count);
extern ssize_t read(int fd, void* buf, size_t count);
extern off_t lseek(int fd, off_t offset, int whence);
extern int close(int fd);
extern char* getcwd(char* buf, size_t size);
extern int fchdir(int fd);
extern int chdir(const char* path);
extern int dup(int oldfd);
extern int dup2(int oldfd, int newfd);