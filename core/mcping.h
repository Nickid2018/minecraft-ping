//
// Created by Nickid2018 on 25-8-5.
//

#ifndef MCPING_H
#define MCPING_H

#include <cJSON.h>
#include <stdbool.h>

#include "mcping_export.h"

typedef struct host_and_port {
    char *host;
    unsigned short port;
} host_and_port;

MCPING_EXPORT cJSON *find_java_mc_server(char *dest, bool srv);

MCPING_EXPORT void print_java_mc_server_info(cJSON *server_info);

MCPING_EXPORT char *filter_text_component(cJSON *component);

MCPING_EXPORT host_and_port find_srv_record(char *dest);

MCPING_EXPORT host_and_port parse_host_and_port(char *dest, int default_port, bool *srv_allowed);

MCPING_EXPORT void set_mcping_verbose(bool verbose);

#endif //MCPING_H
