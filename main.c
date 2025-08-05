#include "args.h"
#include "java.h"

struct arguments arguments;

int main(const int argc, char **argv) {
    arguments.type_flags = 0;
    arguments.srv = true;
    arguments.verbose = false;
    argp_parse(&argp, argc, argv, 0, 0, &arguments);

    if (arguments.type_flags == 0) {
        if (try_java_server(arguments.dest_addr, arguments.srv)) return 0;
        return -1;
    }

    bool success = false;
    if (arguments.type_flags & (TYPE_ALL | TYPE_JE_SERVER) != 0)
        success |= try_java_server(arguments.dest_addr, arguments.srv);
    return success ? 0 : -1;
}
