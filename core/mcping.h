//
// Created by Nickid2018 on 25-8-5.
//

#ifndef MCPING_H
#define MCPING_H

#include <cjson/cJSON.h>
#include <stdbool.h>

#ifdef MCPING_STATIC_DEFINE
#  define MCPING_EXPORT
#  define MCPING_NO_EXPORT
#else
#  ifndef MCPING_EXPORT
#    ifdef mcping_EXPORTS
/* We are building this library */
#      define MCPING_EXPORT __attribute__((visibility("default")))
#    else
/* We are using this library */
#      define MCPING_EXPORT __attribute__((visibility("default")))
#    endif
#  endif

#  ifndef MCPING_NO_EXPORT
#    define MCPING_NO_EXPORT __attribute__((visibility("hidden")))
#  endif
#endif

typedef struct host_and_port {
    char *host;
    unsigned short port;
} host_and_port;

MCPING_EXPORT cJSON *find_java_mc_server(host_and_port dest);

MCPING_EXPORT void print_java_mc_server_info(cJSON *server_info);

MCPING_EXPORT cJSON *find_legacy_mc_server(host_and_port dest);

MCPING_EXPORT void print_legacy_mc_server_info(cJSON *server_info);

MCPING_EXPORT cJSON *find_bedrock_mc_server(host_and_port dest);

MCPING_EXPORT void print_bedrock_mc_server_info(cJSON *server_info);

MCPING_EXPORT char *filter_text_component(cJSON *component);

MCPING_EXPORT host_and_port find_mc_srv_record(char *dest);

MCPING_EXPORT host_and_port parse_host_and_port(char *dest, int default_port, bool *srv_allowed);

MCPING_EXPORT void set_mcping_verbose(bool verbose);

MCPING_EXPORT unsigned char *data_url_to_bytes(char *data_url, unsigned long *out_len);

#endif //MCPING_H
