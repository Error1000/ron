#pragma once
#include "unistd.h"

#define EOF -1

extern int puts(const char* str);
extern void perror(const char* str);


typedef struct{
    int fileno;
} FILE;


static FILE stdin_struct = {STDIN_FILENO};
static FILE stdout_struct = {STDOUT_FILENO};
static FILE stderr_struct = {STDERR_FILENO};

#define stdin (&stdin_struct)
#define stdout (&stdout_struct)
#define stderr (&stderr_struct)

extern FILE* fopen(const char* filename, const char* mode);
extern int fclose(FILE* f);
extern size_t fwrite(const void* buf, size_t size, size_t count, FILE* f);
extern size_t fread(void* buf, size_t size, size_t count, FILE* f);
