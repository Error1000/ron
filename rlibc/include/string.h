#pragma once
#include "stddef.h"
extern size_t strlen(const char* str);
extern int strcmp(const char* str1, const char* str2);
extern char* strstr(const char* str, const char* substr);
extern char* strcat(char* dest, const char* src);
extern char* strcpy(char* dest, const char* src);
extern char* strncpy(char* dest, const char* src, size_t count);
extern char* strchr(const char* str, int ch);
extern char* strtok(char* str, const char* delim);

extern void* memset(void* dest, int ch, size_t count);
extern int memcmp(const void* lhs, const void* rhs, size_t count);
extern void* memcpy(void* dest, const void* src, size_t count);
extern void* memmove(void* dest, const void* src, size_t count);
extern void* memchr(const void* ptr, int ch, size_t count); 
