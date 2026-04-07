// -*- mode: C++; c-file-style: "cc-mode" -*-
//*************************************************************************
// DESCRIPTION: Verilator: Add wires for condition tracking
//
// Code available from: https://verilator.org
//
//*************************************************************************
//
// This pass instruments each IF, COND (ternary), or CASE item with
// an auxiliary wire that stores the result of the condition expression.
// It also emits cond_map.json with a mapping from wire names to
// fully qualified condition expressions.
//
//*************************************************************************

#ifndef VERILATOR_V3ADDCONDPASS_H_
#define VERILATOR_V3ADDCONDPASS_H_

#include "config_build.h"
#include "verilatedos.h"

class AstNetlist;

//######################################################################
// Class V3AddCondPass

class V3AddCondPass final {
public:
    static void run(AstNetlist* rootp);
};


#endif  // VERILATOR_V3ADDCONDPASS_H_
