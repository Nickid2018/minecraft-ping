//
// Created by Nickid2018 on 25-8-5.
//

#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
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
        return &((struct sockaddr_in *) sa)->sin_addr;
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

void write_long(unsigned char *buffer, int64_t value) {
    for (int i = 0; i < 8; ++i) {
        buffer[7 - i] = (unsigned char) (value >> (i * 8) & 0xFF);
    }
}

int64_t read_long(const unsigned char *buffer) {
    int64_t result = 0;
    for (int i = 0; i < 8; ++i) {
        result |= (int64_t) buffer[i] << ((7 - i) * 8);
    }
    return result;
}

bool get_server_info(host_and_port dest, int socket_type, struct addrinfo **server_info) {
    struct addrinfo hints = {0};
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = socket_type;
    int rv = getaddrinfo(
        dest.host, g_strdup_printf("%d", dest.port),
        &hints, server_info
    );
    if (rv != 0) {
        verbose("[Network] Get addr info failed for %s: %s", dest.host, gai_strerror(rv));
        return false;
    }
    return true;
}

int make_tcp_socket(host_and_port dest) {
    struct addrinfo *servinfo, *p = NULL;

    if (!get_server_info(dest, SOCK_STREAM, &servinfo)) return -1;

    int fd = -1;
    char conn_buf[INET6_ADDRSTRLEN];
    for (p = servinfo; p != NULL; p = p->ai_next) {
        if ((fd = socket(p->ai_family, p->ai_socktype, p->ai_protocol)) == -1) {
            verbose("[Network] Try to build socket failed: %s", strerror(errno));
            continue;
        }

        inet_ntop(p->ai_family, get_in_addr(p->ai_addr), conn_buf, sizeof conn_buf);

        long flags = fcntl(fd, F_GETFL, 0);
        if (flags < 0) {
            verbose("[Network] Get flag failed (fnctl): %s", strerror(errno));
            continue;
        }
        if (fcntl(fd, F_SETFL, flags | FNONBLOCK) < 0) {
            verbose("[Network] Set flag NONBLOCK failed (fnctl): %s", strerror(errno));
            continue;
        }

        int res = connect(fd, p->ai_addr, p->ai_addrlen);
        bool success = true;
        if (res < 0) {
            if (errno == EINPROGRESS) {
                struct timeval tv;
                tv.tv_sec = 5;
                tv.tv_usec = 0;
                fd_set set;
                FD_ZERO(&set);
                FD_SET(fd, &set);
                res = select(fd + 1, NULL, &set, NULL, &tv);
                if (res < 0 && errno != EINTR) {
                    verbose("[Network] Error connecting: %s", strerror(errno));
                    success = false;
                } else if (res > 0) {
                    socklen_t opt_len = sizeof(int);
                    int opt_val;
                    if (getsockopt(fd, SOL_SOCKET, SO_ERROR, &opt_val, &opt_len) < 0) {
                        verbose("[Network] Error getsockopt: %s", strerror(errno));
                        success = false;
                        break;
                    }
                    if (opt_val) {
                        verbose("[Network] Error in delayed connection: %s", strerror(opt_val));
                        success = false;
                    }
                } else {
                    verbose("[Network] Connection timeout after 5s");
                    success = false;
                }
            } else {
                verbose("[Network] Error connecting: %s", strerror(errno));
                success = false;
            }
        }

        if (success) {
            flags = fcntl(fd, F_GETFL, 0);
            if (flags < 0) {
                verbose("[Network] Get flag failed (fnctl): %s", strerror(errno));
                close(fd);
                continue;
            }
            if (fcntl(fd, F_SETFL, flags & ~FNONBLOCK) < 0) {
                verbose("[Network] Set flag BLOCK failed (fnctl): %s", strerror(errno));
                close(fd);
                continue;
            }
            break;
        } else {
            close(fd);
        }
    }

    if (p == NULL) {
        verbose("[Network] Failed to connect %s port %d", dest.host, dest.port);
        return -1;
    }

    inet_ntop(p->ai_family, get_in_addr(p->ai_addr), conn_buf, sizeof conn_buf);
    freeaddrinfo(servinfo);
    verbose("[Network] Connected to %s port %d", dest.host, dest.port);

    return fd;
}

unsigned char *data_url_to_bytes(char *data_url, unsigned long *out_len) {
    char *start_point = g_utf8_strchr(data_url, -1, ',');
    if (!start_point)
        start_point = g_utf8_strchr(data_url, -1, ';');
    if (!start_point)
        return NULL;
    start_point += 1;
    return g_base64_decode(start_point, out_len);
}
