#ifndef VERILATOR_V3CONDDUMPPASS_H_
#define VERILATOR_V3CONDDUMPPASS_H_

#include "config_build.h"
#include "verilatedos.h"
#include "V3Global.h"
#include "V3Ast.h"
#include "V3AddCondPass.h"  // For AddedCondWire definition

#include <vector>

class V3CondDumpPass final {
public:
    static void run(AstNetlist* rootp);
};

#endif  // VERILATOR_V3CONDDUMPPASS_H_

