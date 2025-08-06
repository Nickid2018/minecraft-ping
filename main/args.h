//
// Created by Nickid2018 on 25-8-5.
//

#ifndef ARGS_H
#define ARGS_H

#include <argp.h>
#include <stdbool.h>

extern struct argp argp;
extern struct arguments arguments;

#define TYPE_JE_SERVER 0x1
#define TYPE_BE_SERVER 0x2
#define TYPE_LEGACY_SERVER 0x4
#define TYPE_ALL 0x8

struct arguments {
    char *dest_addr;
    int type_flags;

    bool srv;

    bool verbose;
};

#endif //ARGS_H
