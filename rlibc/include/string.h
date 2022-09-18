#pragma once
#include "stddef.h"
extern size_t strlen(const char* str);
extern int strcmp(const char* str1, const char* str2);
extern void* memset(void* dest, int ch, size_t count);
extern int memcmp(const void* lhs, const void* rhs, size_t count);
extern void* memcpy(void* dest, const void* src, size_t count);
extern void* memmove(void* dest, const void* src, size_t count);