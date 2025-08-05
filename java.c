//
// Created by Nickid2018 on 25-8-5.
//

#include <arpa/inet.h>
#include <cJSON.h>
#include <glib.h>
#include <netdb.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>

#include "args.h"
#include "java.h"
#include "network.h"

#define FAIL_FAST \
    if (len == -1) { \
        verbose("[JE] Connection error: %s", strerror(errno)); \
        close(sockfd); \
        return false; \
    }

#define min(a, b) ((a) < (b) ? (a) : (b))

long make_up_packet(char *buffer, host_and_port dest) {
    char buffer_inside[2048];
    int offset = 0;
    buffer_inside[offset++] = '\0'; // Handshake Packet ID
    offset += write_var_int(buffer_inside, offset, 770); // Protocol version
    offset += write_var_int(buffer_inside, offset, strlen(dest.host)); // Host len
    memcpy(buffer_inside + offset, dest.host, strlen(dest.host));
    offset += strlen(dest.host);
    buffer_inside[offset++] = (char) (dest.port >> 8); // port
    buffer_inside[offset++] = (char) (dest.port & 0xFF);
    buffer_inside[offset++] = 1; // state
    int write = write_var_int(buffer, 0, offset);
    memcpy(buffer + write, buffer_inside, offset);
    write += offset;
    return write;
}

bool internal_try(host_and_port dest) {
    struct addrinfo hints = {0}, *servinfo, *p = NULL;
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    verbose("[JE] Try get addr info for %s port %d", dest.host, dest.port);

    int rv = getaddrinfo(
        dest.host, g_strdup_printf("%d", dest.port),
        &hints, &servinfo
    );
    if (rv != 0) {
        verbose("[JE] Get addr info failed for %s: %s", dest.host, gai_strerror(rv));
        return false;
    }

    int sockfd = -1;
    char conn_buf[INET6_ADDRSTRLEN];
    for (p = servinfo; p != NULL; p = p->ai_next) {
        if ((sockfd = socket(p->ai_family, p->ai_socktype, p->ai_protocol)) == -1) {
            verbose("[JE] Try to build socket failed: %s", strerror(errno));
            continue;
        }

        inet_ntop(p->ai_family, get_in_addr(p->ai_addr), conn_buf, sizeof conn_buf);
        verbose("[JE] Attempting connection for %s port %d", dest.host, dest.port);

        if (connect(sockfd, p->ai_addr, p->ai_addrlen) == -1) {
            verbose("[JE] Try to connect failed: %s", strerror(errno));
            close(sockfd);
            continue;
        }

        break;
    }

    if (p == NULL) {
        verbose("[JE] Failed to connect %s port %d", dest.host, dest.port);
        return false;
    }

    inet_ntop(p->ai_family, get_in_addr(p->ai_addr), conn_buf, sizeof conn_buf);
    freeaddrinfo(servinfo);
    verbose("[JE] Connected to %s port %d", dest.host, dest.port);

    char buffer[2048];
    long send_len = make_up_packet(buffer, dest);
    long len = send(sockfd, buffer, send_len, 0);
    FAIL_FAST
    char status_request[2] = {1, 0};
    len = send(sockfd, status_request, 2, 0);
    FAIL_FAST
    verbose("[JE] Sent handshake request");

    GByteArray *byte_array = g_byte_array_new();
    len = recv(sockfd, buffer, 5, 0);
    FAIL_FAST
    int packet_len;

    int offset = read_var_int(buffer, 0, &packet_len); // packet len
    if (offset == -1) {
        verbose("[JE] Invalid packet length header, the server is not a valid Java Server");
        close(sockfd);
        return false;
    }
    g_byte_array_append(byte_array, (guint8 *) (buffer + offset), len - offset);

    while (
        packet_len - byte_array->len > 0 &&
        (len = recv(sockfd, buffer, min(2048, packet_len - byte_array->len), 0)) > 0
    ) {
        g_byte_array_append(byte_array, buffer, len);
    }
    if (packet_len - byte_array->len > 0) {
        verbose("[JE] Read EOF before whole packet receives");
        close(sockfd);
        return false;
    }
    verbose("[JE] Received server data, checking");

    if (byte_array->data[0] != 0) {
        verbose("[JE] Invalid packet id: expect 0, got %d", byte_array->data[0]);
        close(sockfd);
        return false;
    }

    int str_len;
    offset = read_var_int((char *) byte_array->data, 1, &str_len);
    if (offset == -1) {
        verbose("[JE] Invalid string length");
        close(sockfd);
        return false;
    }
    if (packet_len != offset + str_len + 1) {
        verbose("[JE] Invalid packet length: expect %d, got %d", packet_len, offset + str_len + 1);
        close(sockfd);
        return false;
    }

    guint8 zero[1] = {0};
    g_byte_array_append(byte_array, zero, 1);
    char *server_json = g_strdup((char *) (byte_array->data + offset + 1));
    g_byte_array_free(byte_array, TRUE);
    verbose("[JE] Received server JSON string: %s", server_json);

    char ping_packet[10];
    ping_packet[0] = 9;
    ping_packet[1] = 1; // ping
    struct timeval tp;
    gettimeofday(&tp, NULL);
    long ms = tp.tv_sec * 1000 + tp.tv_usec / 1000;
    write_long(ping_packet + 2, ms);
    len = send(sockfd, ping_packet, 10, 0);
    FAIL_FAST
    verbose("[JE] Sent ping request");

    len = recv(sockfd, buffer, 10, 0);
    FAIL_FAST
    if (len != 10) {
        verbose("[JE] Read EOF before whole packet receives", len);
        close(sockfd);
        return false;
    }
    if (buffer[0] != 9 || buffer[1] != 1) {
        verbose(
            "[JE] Invalid pong packet: expect ID 1 with length 9, got ID %d length %d",
            buffer[1], buffer[0]
        );
        close(sockfd);
        return false;
    }
    long pong = read_long(buffer + 2);
    gettimeofday(&tp, NULL);
    ms = tp.tv_sec * 1000 + tp.tv_usec / 1000;
    long ping_time = ms - pong;
    verbose("[JE] Received pong packet, ping time = %d", ping_time);
    close(sockfd);

    cJSON *root = cJSON_Parse(server_json);
    if (root == NULL) {
        const char *error_ptr = cJSON_GetErrorPtr();
        if (error_ptr != NULL)
            verbose("[JE] Invalid server JSON string before %s", error_ptr);
        return false;
    }



    return true;
}

bool try_java_server(char *dest, bool srv) {
    bool srv_allowed;
    host_and_port no_srv = parse_host_and_port(dest, 25565, &srv_allowed);
    if (srv && srv_allowed) {
        host_and_port hap = find_srv_record(dest);
        if (hap.host && internal_try(hap)) return true;
    }
    return internal_try(no_srv);
}
