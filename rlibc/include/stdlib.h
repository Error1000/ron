#pragma once
#include "stddef.h"
#define EXIT_SUCCESS 0
#define EXIT_FAILURE -1

extern void  exit(unsigned int code);
extern void* malloc(size_t size);
extern void  free(void* ptr);