//
// Created by Nickid2018 on 25-8-5.
//

#include <arpa/inet.h>
#include <cJSON.h>
#include <errno.h>
#include <glib.h>
#include <stdio.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>

#include "mcping.h"
#include "utils.h"

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

cJSON *internal_try(host_and_port dest) {
    int sockfd = make_tcp_socket(dest);
    if (sockfd == -1) return NULL;

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
        return NULL;
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
        return NULL;
    }

    if (byte_array->data[0] != 0) {
        verbose("[JE] Invalid packet id: expect 0, got %d", byte_array->data[0]);
        close(sockfd);
        return NULL;
    }

    int str_len;
    offset = read_var_int((char *) byte_array->data, 1, &str_len);
    if (offset == -1) {
        verbose("[JE] Invalid string length at head");
        close(sockfd);
        return NULL;
    }
    if (packet_len != offset + str_len + 1) {
        verbose("[JE] Invalid packet length: expect %d, got %d", packet_len, offset + str_len + 1);
        close(sockfd);
        return NULL;
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
        return NULL;
    }
    if (buffer[0] != 9 || buffer[1] != 1) {
        verbose(
            "[JE] Invalid pong packet: expect ID 1 with length 9, got ID %d length %d",
            buffer[1], buffer[0]
        );
        close(sockfd);
        return NULL;
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
        return NULL;
    }

    cJSON_AddNumberToObject(root, "ping", (double) ping_time);
    return root;
}

cJSON *find_java_mc_server(char *dest, bool srv) {
    bool srv_allowed;
    host_and_port no_srv = parse_host_and_port(dest, 25565, &srv_allowed);
    if (srv && srv_allowed) {
        host_and_port hap = find_srv_record(dest);
        if (hap.host) {
            cJSON *tried = internal_try(hap);
            if (tried) {
                cJSON_AddStringToObject(
                    tried, "srv",
                    g_strdup_printf("%s:%d", hap.host, hap.port)
                );
                return tried;
            }
        }
    }
    return internal_try(no_srv);
}

char *filter_text_component(cJSON *component) {
    if (cJSON_IsString(component))
        return cJSON_GetStringValue(component);
    if (cJSON_IsArray(component)) {
        GPtrArray *array = g_ptr_array_new_with_free_func(g_free);
        for (int i = 0; i < cJSON_GetArraySize(component); i++) {
            cJSON *item = cJSON_GetArrayItem(component, i);
            g_ptr_array_add(array, filter_text_component(item));
        }
        g_ptr_array_add(array, NULL);
        char **strv = (char **) g_ptr_array_steal(array, NULL);
        char *result = g_strjoinv("", strv);
        g_ptr_array_free(array, TRUE);
        return result;
    }
    if (cJSON_IsObject(component)) {
        GPtrArray *array = g_ptr_array_new_with_free_func(g_free);
        if (cJSON_HasObjectItem(component, "extra")) {
            cJSON *extra = cJSON_GetObjectItem(component, "extra");
            for (int i = 0; i < cJSON_GetArraySize(extra); i++) {
                cJSON *item = cJSON_GetArrayItem(extra, i);
                g_ptr_array_add(array, filter_text_component(item));
            }
        }
        if (cJSON_HasObjectItem(component, "text"))
            g_ptr_array_add(array, cJSON_GetStringValue(cJSON_GetObjectItem(component, "text")));
        if (cJSON_HasObjectItem(component, "translatable"))
            g_ptr_array_add(array, cJSON_GetStringValue(cJSON_GetObjectItem(component, "translatable")));
        g_ptr_array_add(array, NULL);
        char **strv = (char **) g_ptr_array_steal(array, NULL);
        char *result = g_strjoinv("", strv);
        g_ptr_array_free(array, TRUE);
        return result;
    }
    return " ";
}

void print_java_mc_server_info(cJSON *server_info) {
    if (server_info == NULL) return;

    server_info = cJSON_Duplicate(server_info, true);

    if (cJSON_HasObjectItem(server_info, "srv")) {
        printf(
            "The server uses SRV Record, request is redirected to %s\n",
            cJSON_GetStringValue(cJSON_GetObjectItem(server_info, "srv"))
        );
        cJSON_DeleteItemFromObject(server_info, "srv");
    }

    if (cJSON_HasObjectItem(server_info, "ping")) {
        printf(
            "Ping to server (Java) is %dms\n",
            (int) cJSON_GetNumberValue(cJSON_GetObjectItem(server_info, "ping"))
        );
        cJSON_DeleteItemFromObject(server_info, "ping");
    }

    if (cJSON_GetObjectItem(server_info, "description")) {
        cJSON *description = cJSON_GetObjectItem(server_info, "description");
        printf("Message Of The Day:\n");
        char *text = filter_text_component(description);
        char **split = g_strsplit(text, "\n", 4);
        char *display = g_strjoinv("\n\t", split);
        g_strfreev(split);
        printf("\t%s\n", display);
        cJSON_DeleteItemFromObject(server_info, "description");
    }

    if (cJSON_HasObjectItem(server_info, "version")) {
        cJSON *version = cJSON_GetObjectItem(server_info, "version");
        cJSON *protocol = cJSON_GetObjectItem(version, "protocol");
        cJSON *name = cJSON_GetObjectItem(version, "name");
        printf("Version:\n");
        printf("\t%-20s: %d\n", "Protocol Version", (int) cJSON_GetNumberValue(protocol));
        printf("\t%-20s: %s\n", "Version Name", cJSON_GetStringValue(name));
        cJSON_DeleteItemFromObject(server_info, "version");
    } else {
        printf("Version:\n\t%-20s: Unknown\n\t%-20s: Unknown\n", "Protocol version", "Version Name");
    }

    if (cJSON_HasObjectItem(server_info, "players")) {
        cJSON *players = cJSON_GetObjectItem(server_info, "players");
        cJSON *online = cJSON_GetObjectItem(players, "online");
        cJSON *max = cJSON_GetObjectItem(players, "max");
        cJSON *sample = cJSON_GetObjectItem(players, "sample");
        printf("Online players:\n");
        printf("\t%-20s: %d\n", "Online Count", (int) cJSON_GetNumberValue(online));
        printf("\t%-20s: %d\n", "Max Players", (int) cJSON_GetNumberValue(max));
        if (sample) {
            int array_count = cJSON_GetArraySize(sample);
            for (int i = 0; i < array_count; i++) {
                cJSON *item = cJSON_GetArrayItem(sample, i);
                char *name = cJSON_GetStringValue(cJSON_GetObjectItem(item, "name"));
                char *id = cJSON_GetStringValue(cJSON_GetObjectItem(item, "id"));
                char *display = strcmp(id, "00000000-0000-0000-0000-000000000000")
                                    ? g_strdup_printf("%-16s (%s)", name, id)
                                    : "Anonymous by server";
                printf("\t%-20s: %s\n", i ? "" : "Sample", display);
            }
        }
        cJSON_DeleteItemFromObject(server_info, "players");
    }

    cJSON_DeleteItemFromObject(server_info, "favicon");

    cJSON *now = server_info->child;
    if (now)
        printf("Non-vanilla Sections:\n");
    while (now) {
        char *name = now->string;
        char *value;
        if (cJSON_IsNull(now)) value = "[Null]";
        else if (cJSON_IsArray(now)) value = "[Array]";
        else if (cJSON_IsObject(now)) value = "[Object]";
        else if (cJSON_IsNumber(now) || cJSON_IsString(now)) value = cJSON_GetStringValue(now);
        else if (cJSON_IsBool(now)) value = cJSON_IsTrue(now) ? "true" : "false";
        else value = "";
        printf("\t%-20s: %s\n", name, value);
        now = now->next;
    }
}
