//
// Created by Nickid2018 on 25-8-5.
//

#include <arpa/inet.h>
#include <errno.h>
#include <glib.h>
#include <netdb.h>
#include <resolv.h>
#include <unistd.h>

#include "utils.h"

bool output_verbose = false;

void set_mcping_verbose(bool verbose) {
    output_verbose = verbose;
}

void verbose(char *fmt, ...) {
    if (!output_verbose) return;
    va_list args;
    va_start(args, fmt);
    vfprintf(stderr, g_strconcat(fmt, "\n", NULL), args);
    va_end(args);
}

GRegex *get_ipv4_regex() {
    return g_regex_new(
        "^((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])$",
        0, 0, NULL
    );
}

host_and_port find_srv_record(char *dest) {
    res_init();

    verbose("[SRV] Start Querying SRV Record for %s", dest);
    unsigned char buffer[4096];
    char *srv_name = g_strconcat("_minecraft._tcp.", dest, NULL);
    int resp = res_query(srv_name, C_IN, T_SRV, buffer, sizeof(buffer));
    if (resp < 0) {
        verbose("[SRV] Query SRV Record failed: %s", hstrerror(h_errno));
        return (host_and_port){NULL, 0};
    }

    ns_msg nsMsg;
    ns_initparse(buffer, resp, &nsMsg);
    for (int x = 0; x < ns_msg_count(nsMsg, ns_s_an); x++) {
        ns_rr rr;
        ns_parserr(&nsMsg, ns_s_an, x, &rr);
        ns_type type = ns_rr_type(rr);
        if (type != ns_t_srv) continue;
        unsigned short port = ntohs(*((unsigned short *) ns_rr_rdata(rr) + 2));
        char host[4096];
        dn_expand(ns_msg_base(nsMsg), ns_msg_end(nsMsg), ns_rr_rdata(rr) + 6, host, sizeof(host));
        verbose("[SRV] Found SRV Record to %s:%d", host, port);
        return (host_and_port){g_strdup(host), port};
    }
    return (host_and_port){NULL, 0};
}

host_and_port parse_host_and_port(char *dest, int default_port, bool *srv_allowed) {
    char *maybe_ipv6 = g_utf8_strchr(dest, -1, ']');
    if (maybe_ipv6) {
        if (srv_allowed) *srv_allowed = false;
        int port = default_port;
        char *only_ipv6 = g_utf8_substring(dest, 1, maybe_ipv6 - dest);
        if (dest[0] != '[')
            return (host_and_port){NULL, 0};
        struct in6_addr addr;
        if (!inet_pton(AF_INET6, only_ipv6, &addr))
            return (host_and_port){NULL, 0};
        if (g_utf8_strlen(maybe_ipv6, 1000) > 1) {
            char should_colon = maybe_ipv6[1];
            if (should_colon != ':') return (host_and_port){NULL, 0};
            char *port_str = maybe_ipv6 + 2;
            errno = 0;
            port = strtol(port_str, NULL, 10);
            if (errno != 0 || port < 0 || port > 65535) return (host_and_port){NULL, 0};
        }
        return (host_and_port){g_strdup(only_ipv6), port};
    }

    int port = default_port;
    char *maybe_port = g_utf8_strchr(dest, -1, ':');
    if (srv_allowed) {
        struct in_addr addr;
        *srv_allowed = !inet_pton(AF_INET, dest, &addr);
    }
    if (maybe_port) {
        errno = 0;
        port = strtol(maybe_port + 1, NULL, 10);
        if (errno != 0 || port < 0 || port > 65535) return (host_and_port){NULL, 0};
        dest = g_utf8_substring(dest, 0, maybe_port - dest);
        if (srv_allowed) *srv_allowed = false;
    }
    return (host_and_port){g_strdup(dest), port};
}

void *get_in_addr(struct sockaddr *sa) {
    if (sa->sa_family == AF_INET) {
        return &(((struct sockaddr_in *) sa)->sin_addr);
    }

    return &((struct sockaddr_in6 *) sa)->sin6_addr;
}

int read_var_int(const char *buffer, int offset, int *result) {
    char read;
    int p = 0;
    *result = 0;
    do {
        if (p == 5)
            return -1;
        read = buffer[offset + p];
        *result |= (read & 0x7F) << (7 * p++);
    } while ((read & 0x80) != 0);
    return p;
}

int write_var_int(char *buffer, int offset, int value) {
    int p = 0;
    while (true) {
        if (value & 0xFFFFFF80) {
            buffer[offset + p++] = (char) (value & 0x7F | 0x80);
            value = value >> 7 & 0x1FFFFFF;
        } else {
            buffer[offset + p++] = (char) (value & 0x7F);
            break;
        }
    }
    return p;
}

void write_long(char *buffer, long value) {
    long *p = (long *) buffer;
    *p = value;
}

long read_long(char *buffer) {
    long *p = (long *) buffer;
    return *p;
}

int make_tcp_socket(host_and_port dest) {
    struct addrinfo hints = {0}, *servinfo, *p = NULL;
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;

    int rv = getaddrinfo(
        dest.host, g_strdup_printf("%d", dest.port),
        &hints, &servinfo
    );
    if (rv != 0) {
        verbose("[Network] Get addr info failed for %s: %s", dest.host, gai_strerror(rv));
        return -1;
    }

    int sockfd = -1;
    char conn_buf[INET6_ADDRSTRLEN];
    for (p = servinfo; p != NULL; p = p->ai_next) {
        if ((sockfd = socket(p->ai_family, p->ai_socktype, p->ai_protocol)) == -1) {
            verbose("[Network] Try to build socket failed: %s", strerror(errno));
            continue;
        }

        inet_ntop(p->ai_family, get_in_addr(p->ai_addr), conn_buf, sizeof conn_buf);
        if (connect(sockfd, p->ai_addr, p->ai_addrlen) == -1) {
            verbose("[Network] Try to connect failed: %s", strerror(errno));
            close(sockfd);
            continue;
        }

        break;
    }

    if (p == NULL) {
        verbose("[Network] Failed to connect %s port %d", dest.host, dest.port);
        return -1;
    }

    inet_ntop(p->ai_family, get_in_addr(p->ai_addr), conn_buf, sizeof conn_buf);
    freeaddrinfo(servinfo);
    verbose("[Network] Connected to %s port %d", dest.host, dest.port);

    return sockfd;
}