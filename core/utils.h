//
// Created by Nickid2018 on 25-8-5.
//

#ifndef NETWORK_H
#define NETWORK_H

#include <netinet/in.h>

#include "mcping.h"

struct addrinfo;

void *get_in_addr(struct sockaddr *sa);

int read_var_int(const char *buffer, int offset, int *result);

int write_var_int(char *buffer, int offset, int value);

void write_long(char unsigned *buffer, int64_t value);

int64_t read_long(const unsigned char *buffer);

bool get_server_info(host_and_port dest, int socket_type, struct addrinfo **server_info);

int make_tcp_socket(host_and_port dest);

int make_udp_send_channel(host_and_port dest, struct sockaddr_in *servaddr);

int make_udp_receive_channel(host_and_port dest, struct sockaddr_in *servaddr);

void verbose(char *, ...);

#endif //NETWORK_H
