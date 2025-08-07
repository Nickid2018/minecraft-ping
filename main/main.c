#include <cJSON.h>
#include <unistd.h>

#include "args.h"
#include "../core/mcping.h"

struct arguments arguments;

#define PRINT(fmt, ...) if (!arguments.fav_to_stdout) printf(fmt, ##__VA_ARGS__);

bool find_and_try_output(host_and_port addr, cJSON *(*find)(host_and_port), void (*display)(cJSON *)) {
    cJSON *data = find(addr);
    if (data != NULL) {
        if (!arguments.fav_to_stdout)
            display(data);
        if (arguments.fav_to_stdout || arguments.fav_output_file) {
            char *favicon = cJSON_GetStringValue(cJSON_GetObjectItem(data, "favicon"));
            if (!favicon) return true;
            unsigned long size;
            unsigned char *buf = data_url_to_bytes(favicon, &size);
            FILE *fp = arguments.fav_to_stdout
                           ? stdout
                           : fopen(arguments.fav_output_file, "wb");
            if (!fp) {
                perror("File opening failed");
                return true;
            }
            fwrite(buf, 1, size, fp);
            if (ferror(fp)) {
                perror("Writing failed");
                return true;
            }
            fclose(fp);
        }
        return true;
    }
    return false;
}

int main(const int argc, char **argv) {
    arguments.type_flags = 0;
    arguments.srv = true;
    arguments.verbose = false;
    argp_parse(&argp, argc, argv, 0, 0, &arguments);

    bool success_return = arguments.type_flags == 0 || arguments.fav_to_stdout;
    if (!arguments.type_flags || arguments.type_flags & TYPE_ALL)
        arguments.type_flags = -1;
    set_mcping_verbose(arguments.verbose && !arguments.fav_to_stdout);

    bool success = false;
    if (arguments.type_flags & TYPE_JE_SERVER) {
        bool srv_allowed;
        host_and_port no_srv = parse_host_and_port(arguments.dest_addr, 25565, &srv_allowed);
        if (arguments.srv && srv_allowed) {
            host_and_port srv = find_srv_record(arguments.dest_addr);
            if (srv.host) {
                PRINT(
                    "The server uses SRV Record, try to find server at %s:%d\n",
                    srv.host, srv.port
                );
                bool suc = find_and_try_output(srv, find_java_mc_server, print_java_mc_server_info);
                if (suc && success_return) return 0;
                success |= suc;
                if (suc) {
                    no_srv.host = NULL;
                } else {
                    PRINT("SRV redirection is invalid, try find server at original address\n");
                }
            }
        }

        if (no_srv.host) {
            bool suc = find_and_try_output(no_srv, find_java_mc_server, print_java_mc_server_info);
            if (suc && success_return) return 0;
            success |= suc;
        }

        if (!success && !arguments.fav_to_stdout)
            printf("No Java Server found\n");
    }

    if (arguments.type_flags & TYPE_BE_SERVER) {
        host_and_port addr = parse_host_and_port(arguments.dest_addr, 19132, NULL);
        bool suc = find_and_try_output(addr, find_bedrock_mc_server, print_bedrock_mc_server_info);
        if (suc && success_return) return 0;
        success |= suc;
        if (!success && !arguments.fav_to_stdout)
            printf("No Bedrock Server found\n");
    }

    return success ? 0 : -1;
}
