#pragma once
#include "unistd.h"
#include "stdarg.h"
const int EOF = -1;

extern int puts(const char* str);
extern void perror(const char* str);

typedef struct{
    int fileno;
} FILE;


FILE stdin_struct = {STDIN_FILENO};
FILE stdout_struct = {STDOUT_FILENO};
FILE stderr_struct = {STDERR_FILENO};

#define stdin (&stdin_struct)
#define stdout (&stdout_struct)
#define stderr (&stderr_struct)


extern FILE* fopen(const char* filename, const char* mode);
extern int fclose(FILE* f);
extern size_t fwrite(const void* buf, size_t size, size_t count, FILE* f);
extern size_t fread(void* buf, size_t size, size_t count, FILE* f);
extern int fseek(FILE* f, long offset, int origin);

extern int vfprintf(FILE* out_stream, const char* format, va_list vlist);
int vprintf(const char* format, va_list vlist) { return vfprintf(stdout, format, vlist); } 


int printf(const char* format, ...) {
    va_list args;
    va_start(args, format);
    int res = vprintf(format, args);
    va_end(args);
    return res;
}

int fprintf(FILE* out_stream, const char* format, ...) {
    va_list args;
    va_start(args, format);
    int res = vfprintf(out_stream, format, args);
    va_end(args);
    return res;
}

extern int vfscanf(FILE* in_stream, const char* format, va_list vlist);
int vscanf(const char* format, va_list vlist) { return vfscanf(stdin, format, vlist); }

int scanf(const char* format, ...) {
    va_list args;
    va_start(args, format);
    int res = vscanf(format, args);
    va_end(args);
    return res;
}

int fscanf(FILE* in_stream, const char* format, ...){
    va_list args;
    va_start(args, format);
    int res = vfscanf(in_stream, format, args);
    va_end(args);
    return res;
}

extern int fputc(int ch, FILE* f);

extern char* fgets(char* str, int count, FILE* f);
extern int fputs(const char* str, FILE* f);

// putc() may be implemented as a macro
// Source: https://en.cppreference.com/w/c/io/fputc
#define putc(ch, f) fputc(ch, f)

// Equivalent to putc(ch, stdout). 
// Source: https://en.cppreference.com/w/c/io/putchar
#define putchar(ch) putc(ch, stdout)


extern int fgetc(FILE* f);

// Same as fgetc, except that if getc is implemented as a macro, it may evaluate stream more than once
// Source: https://en.cppreference.com/w/c/io/fgetc
#define getc(f) fgetc(f)

// Equivalent to getc(stdin). 
// Source: https://en.cppreference.com/w/c/io/getchar
#define getchar() getc(stdin)