//
// Created by Nickid2018 on 25-8-5.
//

#ifndef NETWORK_H
#define NETWORK_H

#include <netinet/in.h>

#include "mcping.h"

void *get_in_addr(struct sockaddr *sa);

int read_var_int(const char *buffer, int offset, int *result);

int write_var_int(char *buffer, int offset, int value);

void write_long(char *buffer, long value);

long read_long(char *buffer);

int make_tcp_socket(host_and_port dest);

void verbose(char *, ...);

#endif //NETWORK_H
