//
// Created by Nickid2018 on 25-8-5.
//

#ifndef NETWORK_H
#define NETWORK_H

#include <netinet/in.h>

typedef struct host_and_port {
    char *host;
    unsigned short port;
} host_and_port;

host_and_port find_srv_record(char *dest);

host_and_port parse_host_and_port(char *dest, int default_port, bool *srv_allowed);

void *get_in_addr(struct sockaddr *sa);

int read_var_int(const char *buffer, int offset, int *result);

int write_var_int(char *buffer, int offset, int value);

void write_long(char *buffer, long value);

long read_long(char *buffer);

#endif //NETWORK_H
