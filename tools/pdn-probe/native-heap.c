/* Developer-only Linux allocator for executing original Windows reference IL.
 * Serialization and document routines are not modified. Never shipped in the editor.
 */
#include <stdlib.h>
#include <stdint.h>
#include <unistd.h>
#include <sys/syscall.h>
void *GetProcessHeap(void) { return (void *)1; }
void *HeapAlloc(void *heap, uint32_t flags, size_t size) {
    (void)heap;
    size_t *base = (flags & 8) ? calloc(1,size+sizeof(size_t)) : malloc(size+sizeof(size_t));
    if (!base) return NULL;
    *base=size; return base+1;
}
size_t HeapSize(void *heap, uint32_t flags, void *memory) {
    (void)heap; (void)flags; return ((size_t *)memory)[-1];
}
int HeapFree(void *heap, uint32_t flags, void *memory) {
    (void)heap; (void)flags; free((size_t *)memory-1); return 1;
}
void *VirtualAlloc(void *address, size_t size, uint32_t type, uint32_t protection) {
    (void)address; (void)type; (void)protection; return calloc(1,size);
}
int VirtualFree(void *memory, size_t size, uint32_t type) {
    (void)size; (void)type; free(memory); return 1;
}
uint32_t GetCurrentThreadId(void) { return (uint32_t)syscall(SYS_gettid); }
uint32_t GetCurrentProcessId(void) { return (uint32_t)getpid(); }
