long write(long fd, const void *buf, long count) {
    long ret;
    asm volatile(
        "syscall"
        : "=a"(ret)
        : "a"(1), "D"(fd), "S"(buf), "d"(count)
        : "rcx", "r11"
    );
    return ret;
}

long exit(long code) {
    asm("syscall" : : "a"(60), "D"(code));
    return 0;
}

void _start() {
    write(1, "hllo world", 10);
    exit(0);
}
