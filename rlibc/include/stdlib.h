#pragma once
#include "stddef.h"
#define EXIT_SUCCESS 0
#define EXIT_FAILURE -1

extern void  exit(unsigned int code);
extern void* malloc(size_t size);
extern void* realloc(void* ptr, size_t new_size);
extern void  free(void* ptr);
extern char* getenv(const char* name);
