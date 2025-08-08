//
// Created by Nickid2018 on 25-8-5.
//

#include <errno.h>
#include <glib.h>
#include <netdb.h>
#include <stdio.h>
#include <string.h>
#include <sys/time.h>
#include <unistd.h>

#include "mcping.h"
#include "utils.h"

const int64_t MAGIC_HIGH = 0x00ffff00fefefefeL;
const int64_t MAGIC_LOW = 0xfdfdfdfd12345678L;

cJSON *find_bedrock_mc_server(host_and_port dest) {
    struct addrinfo *server_info;
    if (!get_server_info(dest, SOCK_DGRAM, &server_info))
        return NULL;

    int fd = socket(server_info->ai_family, server_info->ai_socktype, server_info->ai_protocol);
    if (fd < 0) {
        verbose("[BE] Can't open UDP socket: %s", strerror(errno));
        return NULL;
    }

    struct timeval timeout;
    timeout.tv_sec = 10;
    timeout.tv_usec = 0;
    if (setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout)) < 0) {
        verbose("[BE] Can't set socket timeout: %s", strerror(errno));
        close(fd);
        return NULL;
    }

    struct sockaddr_in serve_addr;
    bzero(&serve_addr, sizeof(serve_addr));
    serve_addr.sin_family = AF_INET;
    serve_addr.sin_addr.s_addr = INADDR_ANY;
    serve_addr.sin_port = htons(11451);
    if (bind(fd, (const struct sockaddr *) &serve_addr, sizeof(serve_addr)) < 0) {
        verbose("[BE] Can't bind UDP socket: %s", strerror(errno));
        close(fd);
        return NULL;
    }

    unsigned char req[27];
    req[0] = 1; // Packet ID 0x1 (Unconnected Ping, Raknet Protocol)

    struct timeval tp;
    gettimeofday(&tp, NULL);
    long send_time = tp.tv_sec * 1000 + tp.tv_usec / 1000;
    write_long(req + 1, send_time);
    write_long(req + 9, MAGIC_HIGH);
    write_long(req + 17, MAGIC_LOW);
    req[25] = 0; // Client GUID, 0
    req[26] = 0; // terminator

    if (sendto(fd, req, 27, 0, server_info->ai_addr, sizeof(*server_info->ai_addr)) < 0) {
        verbose("[BE] Error sending request: %s", strerror(errno));
        close(fd);
        return NULL;
    }
    verbose("[BE] Sent status request");

    unsigned char buffer[2048];
    ssize_t len;
    struct sockaddr sender_addr;
    socklen_t socket_len;
    if ((len = recvfrom(fd, buffer, 2048, 0, &sender_addr, &socket_len)) < 0) {
        verbose("[BE] Error receiving response: %s", strerror(errno));
        return NULL;
    }
    close(fd);
    buffer[len] = 0;

    if (buffer[0] != 0x1c) {
        verbose("[BE] Bad response header from server, expect 28, got %d", buffer[0]);
        return NULL;
    }

    gettimeofday(&tp, NULL);
    long time_now = tp.tv_sec * 1000 + tp.tv_usec / 1000;
    uint64_t ping_time = read_long(buffer + 1);
    verbose("[BE] Received ping: %ldms", time_now - ping_time);

    uint64_t server_guid = read_long(buffer + 9);
    uint64_t magic_high = read_long(buffer + 17);
    uint64_t magic_low = read_long(buffer + 25);
    ushort str_len = (buffer[33] << 8) + buffer[34];
    if (magic_high != MAGIC_HIGH) {
        verbose("[BE] Bad Magic Number High bits, expect %x, got %x", MAGIC_HIGH, magic_high);
        return NULL;
    }
    if (magic_low != MAGIC_LOW) {
        verbose("[BE] Bad Magic Number Low bits, expect %x, got %x", MAGIC_LOW, magic_low);
        return NULL;
    }
    if (str_len != len - 35) {
        verbose("[BE] Bad String Length, expect %d, got %d", len - 35, str_len);
        return NULL;
    }

    cJSON *root = cJSON_CreateObject();
    cJSON_AddNumberToObject(root, "ping", (double) (time_now - ping_time));
    cJSON_AddStringToObject(root, "server_guid", g_strdup_printf("%ld", server_guid));

    verbose("[BE] Received server response: %s", buffer + 35);
    char **split = g_regex_split(
        g_regex_new(";", 0, 0, NULL),
        (char *) buffer + 35, 0
    );

    int i = 0;
    for (; split[i]; i++) {
        char *name;
        switch (i) {
            case 0:
                name = "edition";
                break;
            case 1:
                name = "motd1";
                break;
            case 2:
                name = "protocol";
                break;
            case 3:
                name = "version";
                break;
            case 4:
                name = "players";
                break;
            case 5:
                name = "maxPlayers";
                break;
            case 6:
                name = "";
                break;
            case 7:
                name = "motd2";
                break;
            case 8:
                name = "gameMode";
                break;
            default:
                name = NULL;
        }
        if (name == NULL) {
            verbose("[BE] Invalid response? Data will be collected, but can not ensure data is correct");
        } else {
            cJSON_AddStringToObject(root, name, split[i]);
        }
    }
    if (i < 8) {
        verbose("[BE] Response is corrupted? Data will be collected, but can not ensure data is correct");
    }
    return root;
}

char *remove_format_char(char *source) {
    return g_regex_replace(
        g_regex_new("\u00a7.", 0, 0, NULL),
        source, strlen(source),
        0, "", 0, NULL
    );
}

void print_bedrock_mc_server_info(cJSON *server_info) {
    if (server_info == NULL) return;

    if (cJSON_HasObjectItem(server_info, "ping")) {
        printf(
            "Ping to server (Bedrock) is %dms\n",
            cJSON_GetObjectItem(server_info, "ping")->valueint
        );
    }

    if (cJSON_GetObjectItem(server_info, "motd1")) {
        cJSON *motd1 = cJSON_GetObjectItem(server_info, "motd1");
        cJSON *motd2 = cJSON_GetObjectItem(server_info, "motd2");
        printf("Message Of The Day:\n");
        printf("\t%s\n", remove_format_char(cJSON_GetStringValue(motd1)));
        printf("\t%s\n", remove_format_char(cJSON_GetStringValue(motd2)));
    }

    if (cJSON_HasObjectItem(server_info, "version")) {
        cJSON *version = cJSON_GetObjectItem(server_info, "version");
        cJSON *protocol = cJSON_GetObjectItem(server_info, "protocol");
        printf("Version:\n");
        printf("\t%-20s: %s\n", "Protocol Version", cJSON_GetStringValue(protocol));
        printf("\t%-20s: %s\n", "Version Name", cJSON_GetStringValue(version));
    } else {
        printf("Version:\n\t%-20s: Unknown\n\t%-20s: Unknown\n", "Protocol version", "Version Name");
    }

    if (cJSON_HasObjectItem(server_info, "players")) {
        cJSON *online = cJSON_GetObjectItem(server_info, "players");
        cJSON *max = cJSON_GetObjectItem(server_info, "maxPlayers");
        printf("Online players:\n");
        printf("\t%-20s: %s\n", "Online Count", cJSON_GetStringValue(online));
        printf("\t%-20s: %s\n", "Max Players", cJSON_GetStringValue(max));
    }

    if (cJSON_HasObjectItem(server_info, "server_guid")) {
        cJSON *guid = cJSON_GetObjectItem(server_info, "server_guid");
        printf("Server GUID:\n\t%s\n", cJSON_GetStringValue(guid));
    }

    if (cJSON_HasObjectItem(server_info, "gameMode")) {
        cJSON *mode = cJSON_GetObjectItem(server_info, "gameMode");
        printf("Server Game Mode:\n\t%s\n", cJSON_GetStringValue(mode));
    }
}
