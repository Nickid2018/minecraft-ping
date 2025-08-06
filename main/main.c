#include <cJSON.h>

#include "args.h"
#include "../core/mcping.h"

struct arguments arguments;

int main(const int argc, char **argv) {
    arguments.type_flags = 0;
    arguments.srv = true;
    arguments.verbose = false;
    argp_parse(&argp, argc, argv, 0, 0, &arguments);

    set_mcping_verbose(arguments.verbose);

    if (arguments.type_flags == 0) {
        cJSON * data = find_java_mc_server(arguments.dest_addr, arguments.srv);
        if (data != NULL) {
            print_java_mc_server_info(data);
            return 0;
        }
        return -1;
    }

    bool success = false;
    if ((arguments.type_flags & (TYPE_ALL | TYPE_JE_SERVER)) != 0) {
        cJSON * data = find_java_mc_server(arguments.dest_addr, arguments.srv);
        if (data != NULL) {
            print_java_mc_server_info(data);
            success = true;
        }
    }
    return success ? 0 : -1;
}
