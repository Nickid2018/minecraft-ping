//
// Created by Nickid2018 on 25-8-5.
//
#include "args.h"

#include <glib.h>
#include <string.h>

#include "../core/mcping.h"

#define ARG_KEY_NO_SRV 0x100001
#define ARG_KEY_FAV_TO_STDOUT 0x100002

const char *argp_program_version = "MCPing 1.0";
static char doc[] = "Ping to a Minecraft Server";
static char args_doc[] = "DESTADDR";

static struct argp_option options[] = {
    {"type", 't', "java|je|bedrock|be|legacy|all", OPTION_ARG_OPTIONAL, "Ping server type"},
    {"nosrv", ARG_KEY_NO_SRV, 0, OPTION_ARG_OPTIONAL, "Do not lookup SRV Record"},
    {"favicon", 'f', "FILE", OPTION_ARG_OPTIONAL, "Output favicon to file"},
    {"favicon-out", ARG_KEY_FAV_TO_STDOUT, 0, OPTION_ARG_OPTIONAL, "Output favicon to standard output"},
    {"fo", ARG_KEY_FAV_TO_STDOUT, 0, OPTION_ARG_OPTIONAL | OPTION_ALIAS, "Output favicon to standard output"},
    {"verbose", 'v', 0, OPTION_ARG_OPTIONAL, "Verbose output"},
    {0}
};

static error_t parse_opt(int key, char *arg, struct argp_state *state) {
    struct arguments *args = state->input;
    switch (key) {
        case 't':
            if (arg == NULL) {
                argp_error(state, "Invalid type option: not specified type");
                break;
            }
            if (strcmp(arg, "bedrock") == 0 || strcmp(arg, "be") == 0) {
                args->type_flags |= TYPE_BE_SERVER;
                break;
            }
            if (strcmp(arg, "java") == 0 || strcmp(arg, "je") == 0) {
                args->type_flags |= TYPE_JE_SERVER;
                break;
            }
            if (strcmp(arg, "legacy") == 0) {
                args->type_flags |= TYPE_LEGACY_SERVER;
                break;
            }
            if (strcmp(arg, "all") == 0) {
                args->type_flags |= TYPE_ALL;
                break;
            }
            argp_error(state, "Invalid type option: %s", arg);
            break;
        case 'v':
            args->verbose = true;
            break;
        case 'f':
            if (args->fav_to_stdout)
                argp_error(state, "-f and --fo can't coexist");
            args->fav_output_file = arg;
            break;
        case ARG_KEY_NO_SRV:
            args->srv = false;
            break;
        case ARG_KEY_FAV_TO_STDOUT:
            if (args->fav_output_file)
                argp_error(state, "-f and --fo can't coexist");
            args->fav_to_stdout = true;
            break;
        case ARGP_KEY_ARG:
            if (state->arg_num >= 1)
                argp_error(state, "Too many arguments");
            host_and_port test = parse_host_and_port(arg, 0, NULL);
            if (!test.host)
                argp_error(state, "Invalid address");
            args->dest_addr = arg;
            break;
        case ARGP_KEY_END:
            if (state->arg_num < 1)
                argp_error(state, "Too few arguments");
            break;
        default:
            return ARGP_ERR_UNKNOWN;
    }
    return 0;
}

struct argp argp = {options, parse_opt, args_doc, doc};
