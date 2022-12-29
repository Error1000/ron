#pragma once
#define O_RDONLY 0b00001
#define O_WRONLY 0b00010
#define O_RDWR   (O_RDONLY | O_WRONLY)
#define O_APPEND 0b00100
#define O_CREAT  0b01000
#define O_TRUNC  0b10000

extern int open(const char* pathname, int flags);
