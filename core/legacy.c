//
// Created by Nickid2018 on 25-8-5.
//

#include <errno.h>
#include <glib.h>
#include <iconv.h>
#include <stdio.h>
#include <string.h>
#include <uchar.h>
#include <unistd.h>
#include <sys/time.h>

#include "mcping.h"
#include "utils.h"

unsigned char LEGACY_QUERY_HEADER[27] = {
    0xFE, 0x01, 0xFA, 0x00, 0x0B, 0x00, 0x4D, 0x00, 0x43,
    0x00, 0x7C, 0x00, 0x50, 0x00, 0x69, 0x00, 0x6E, 0x00,
    0x67, 0x00, 0x48, 0x00, 0x6F, 0x00, 0x73, 0x00, 0x74
};

size_t make_up_utf16_be(char16_t *out, const char *buffer, size_t size) {
    const char *p_in = buffer;
    const char *end = buffer + size;
    char16_t *p_out = out;
    mbstate_t state = {0};

    for (size_t rc; (rc = mbrtoc16(p_out, p_in, end - p_in, &state));) {
        if (rc == (size_t) -1) // invalid input
            break;
        if (rc == (size_t) -2) // truncated input
            break;
        if (rc == (size_t) -3) // UTF-16 high surrogate
            p_out += 1;
        else {
            p_in += rc;
            p_out += 1;
        }
    }

    return p_out - out;
}

size_t make_up_legacy_packet(unsigned char *buffer, host_and_port dest) {
    size_t offset = 27;
    memcpy(buffer, LEGACY_QUERY_HEADER, 27);

    char16_t utf16[1024];
    size_t utf16_len = make_up_utf16_be(utf16, dest.host, strlen(dest.host));
    size_t name_len = utf16_len * 2;
    size_t whole_len = name_len + 7;

    buffer[offset++] = (unsigned char) (whole_len >> 8 & 0xFF);
    buffer[offset++] = (unsigned char) (whole_len & 0xFF);
    buffer[offset++] = 73;

    buffer[offset++] = (unsigned char) (utf16_len >> 8 & 0xFF);
    buffer[offset++] = (unsigned char) (utf16_len & 0xFF);
    for (int i = 0; i < utf16_len; i++) {
        buffer[offset++] = (unsigned char) (utf16[i] >> 8 & 0xFF);
        buffer[offset++] = (unsigned char) (utf16[i] & 0xFF);
    }

    buffer[offset++] = 0;
    buffer[offset++] = 0;
    buffer[offset++] = (unsigned char) (dest.port >> 8); // port
    buffer[offset++] = (unsigned char) (dest.port & 0xFF);

    return offset;
}

#define FAIL_FAST \
    if (len == -1) { \
        verbose("[Legacy] Connection error: %s", strerror(errno)); \
        close(sockfd); \
        return false; \
    }

#define min(a, b) ((a) < (b) ? (a) : (b))

cJSON *find_legacy_mc_server(host_and_port dest) {
    int sockfd = make_tcp_socket(dest);
    if (sockfd == -1) return NULL;

    unsigned char buffer[2048];
    size_t send_len = make_up_legacy_packet(buffer, dest);
    struct timeval tp;
    gettimeofday(&tp, NULL);
    long send_time = tp.tv_sec * 1000 + tp.tv_usec / 1000;
    long len = send(sockfd, buffer, send_len, 0);
    FAIL_FAST

    len = recv(sockfd, buffer, 3, 0);
    FAIL_FAST

    gettimeofday(&tp, NULL);
    long recv_time = tp.tv_sec * 1000 + tp.tv_usec / 1000;

    if (buffer[0] != 0xFF) {
        verbose("[Legacy] Invalid legacy response header, expect 0xFF, got 0x%x", buffer[0]);
        close(sockfd);
        return NULL;
    }

    GByteArray *byte_array = g_byte_array_new();
    int packet_len = (buffer[1] << 8 | buffer[2]) * 2;
    while (
        packet_len - byte_array->len > 0 &&
        (len = recv(sockfd, buffer, min(2048, packet_len - byte_array->len), 0)) > 0
    ) {
        g_byte_array_append(byte_array, buffer, len);
    }
    if (packet_len - byte_array->len > 0) {
        verbose("[Legacy] Read EOF before whole packet receives");
        close(sockfd);
        return NULL;
    }
    close(sockfd);

    verbose("[Legacy] Received server info");
    if (packet_len < 3) {
        verbose("[Legacy] Invalid legacy response length");
        return NULL;
    }

    iconv_t cd = iconv_open("UTF-8", "UTF-16BE");
    if (cd == (iconv_t) -1) {
        verbose("[Legacy] Failed to convert UTF-16BE to UTF-8: %s", strerror(errno));
        return NULL;
    }

    char *input = (char *) byte_array->data;
    size_t in_len = byte_array->len;
    char *output = malloc(byte_array->len * 2);
    char *output_end = output;
    size_t out_len = byte_array->len * 2;
    if (
        iconv(
            cd,
            &input, &in_len,
            &output_end, &out_len
        ) == (size_t) -1
    ) {
        verbose("[Legacy] Failed to convert UTF-16BE to UTF-8: %s", strerror(errno));
        iconv_close(cd);
        free(output);
        return NULL;
    }

    iconv_close(cd);
    *output_end = '\0';

    cJSON *root = cJSON_CreateObject();
    if (
        // U+00A7 encoded to "C2 A7" in UTF-8
        output[0] == (char) 0xC2 && output[1] == (char) 0xA7 &&
        output[2] == 0x31 && output[3] == 0x00
    ) {
        char *read = output + 4;
        verbose("[Legacy] Version 1 response");
        cJSON_AddNumberToObject(root, "resp_version", 1);
        int count = 0;
        for (; read < output_end; count++) {
            char *name;
            switch (count) {
                case 0:
                    name = "protocol";
                    break;
                case 1:
                    name = "version";
                    break;
                case 2:
                    name = "motd";
                    break;
                case 3:
                    name = "players";
                    break;
                case 4:
                    name = "maxPlayers";
                    break;
                default:
                    name = NULL;
                    verbose("[Legacy] Invalid response? Data will be collected, but can not ensure data is correct");
            }
            cJSON_AddStringToObject(root, name, g_strdup(read));
            read += strlen(read) + 1;
        }
        if (count < 4) {
            verbose("[Legacy] Response is corrupted? Data will be collected, but can not ensure data is correct");
        }
    } else {
        verbose("[Legacy] Version 0 response");
        cJSON_AddNumberToObject(root, "resp_version", 0);
        char **split = g_strsplit(output, "\u00A7", 4);
        cJSON_AddStringToObject(root, "motd", g_strdup(split[0]));
        cJSON_AddStringToObject(root, "players", g_strdup(split[1]));
        cJSON_AddStringToObject(root, "maxPlayers", g_strdup(split[2]));
        g_strfreev(split);
    }
    cJSON_AddNumberToObject(root, "ping", (double) (recv_time - send_time));

    free(output);
    return root;
}

void print_legacy_mc_server_info(cJSON *server_info) {
    if (cJSON_HasObjectItem(server_info, "ping")) {
        printf(
            "Ping to server (Legacy) is %dms\n",
            (int) cJSON_GetNumberValue(cJSON_GetObjectItem(server_info, "ping"))
        );
    }

    if (cJSON_GetObjectItem(server_info, "motd")) {
        cJSON *motd = cJSON_GetObjectItem(server_info, "motd");
        printf("Message Of The Day:\n");
        printf("\t%s\n", cJSON_GetStringValue(motd));
    }

    printf("Version:\n");
    printf(
        "\t%-20s: %d\n",
        "Response Version",
        (int) cJSON_GetNumberValue(cJSON_GetObjectItem(server_info, "resp_version"))
    );

    if (cJSON_HasObjectItem(server_info, "protocol")) {
        cJSON *version = cJSON_GetObjectItem(server_info, "version");
        cJSON *protocol = cJSON_GetObjectItem(server_info, "protocol");
        printf("\t%-20s: %s\n", "Protocol Version", cJSON_GetStringValue(protocol));
        printf("\t%-20s: %s\n", "Version Name", cJSON_GetStringValue(version));
    }

    if (cJSON_HasObjectItem(server_info, "players")) {
        cJSON *online = cJSON_GetObjectItem(server_info, "players");
        cJSON *max = cJSON_GetObjectItem(server_info, "maxPlayers");
        printf("Online players:\n");
        printf("\t%-20s: %s\n", "Online Count", cJSON_GetStringValue(online));
        printf("\t%-20s: %s\n", "Max Players", cJSON_GetStringValue(max));
    }
}
