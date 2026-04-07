// src/V3AddCondPass.cpp
#include "V3PchAstNoMT.h"
#include "V3AddCondPass.h"

#include "V3Ast.h"
#include "V3Global.h"
#include "V3Stats.h"
#include "V3Const.h"

#include <unordered_map>
#include <fstream>
#include <sstream>

VL_DEFINE_DEBUG_FUNCTIONS;

namespace {
class CondTraceVisitor final : public VNVisitor {
    std::unordered_map<std::string,int> m_locCounts;

    static std::string sanitizeId(std::string s) {
        for (char& c: s) {
            if (!((c>='A'&&c<='Z')||(c>='a'&&c<='z')||(c>='0'&&c<='9')||c=='_'||c=='$'))
                c = '_';
        }
        return s;
    }

    static inline bool isSynthModule(AstNodeModule* modp) {
        return modp && VN_IS(modp, Module);
    }

    // Returns the enclosing synthesizable module for a node, or nullptr if
    // the node is inside a function, task, or package (i.e., non-module scope).
    static AstModule* safeEnclosingModule(AstNode* n) {
        AstNodeModule* modp = nullptr;

        for (AstNode* p = n; p; p = p->backp()) {
            // Stop if we enter any non-module scope (functions, tasks, packages)
            if (VN_IS(p, NodeFTask) || VN_IS(p, Package)) {
                return nullptr;
            }

            // This finds both AstModule and AstIface
            if (VN_IS(p, NodeModule)) {
                modp = VN_CAST(p, NodeModule);
                break;
            }

            // Also stop at generates – we don’t want to cross generate boundaries
            if (VN_IS(p, GenBlock) || VN_IS(p, GenIf) || VN_IS(p, GenCase)) {
                // keep walking, but note that inserted wires shouldn't go here
                continue;
            }
        }

        // Only return real synthesizable modules
        if (modp && VN_IS(modp, Module)) {
            AstModule* realModp = VN_CAST(modp, Module);
            // if (realModp->isTop() || ()) {
                return realModp;
            // }
        }

        return nullptr;
    }


    // // Find the proper insertion point for new statements
    // static AstNode* findInsertionPoint(AstNode* nodep) {
    //     // Walk up to find a statement context where we can insert new nodes
    //     std::cout << "Finding insertion point for " << nodep->name() << std::endl;
    //     for (AstNode* p = nodep; p; p = p->backp()) {
    //         std::cout << "  Checking " << p->name() << std::endl;
    //         if (VN_IS(p, Always) || VN_IS(p, Initial) || VN_IS(p, NodeModule)) {
    //             return p;
    //         }
    //     }
    //     return nullptr;
    // }

    std::string makeName(FileLine* fl, const char* prefix) {
        std::string loc = sanitizeId(fl->filename()) + "_" + cvtToStr(fl->lineno());
        int& count = m_locCounts[loc];
        return std::string(prefix) + "_" + loc + "_" + cvtToStr(count++);
    }

    std::string makeQualifiedName(AstNodeModule* modp, const std::string& wireName) {
        return modp->name() + "." + wireName;
    }

    AstVar* declareWire(FileLine* fl, AstNodeModule* modp, const std::string& name) {
        UINFO(6, "Declaring wire " << name << " in module " << modp->name() << std::endl);
        AstVar* varp = new AstVar(fl, VVarType::WIRE, name, VFlagLogicPacked(), 1);
        varp->sigPublic(true);
        varp->trace(true);
        
        // Insert at module level - this should be safe for variable declarations
        if (modp->stmtsp()) {
            modp->stmtsp()->addHereThisAsNext(varp);
        } else {
            modp->addStmtsp(varp);
        }
        return varp;
    }

    void assignCondStoreWire(FileLine* fl, AstNodeModule* modp, AstVar* lhsVar, 
                           AstNodeExpr* rhsCond) {
        UINFO(6, "Assigning condition to wire " << lhsVar->name() << std::endl);
        
        // Clone the condition for the assignment
        AstNodeExpr* rhsExpr = rhsCond->cloneTree(true);
        AstNodeExpr* normCond = new AstRedOr(fl, rhsExpr);
        
        // Create the assignment
        AstVarRef* lhs = new AstVarRef(fl, lhsVar, VAccess::WRITE);
        AstAssignW* asn = new AstAssignW(fl, lhs, normCond);
       
        // Mark the VARIABLE using the tag field - tags belong on AstVar, not AstAssignW
        lhsVar->tag("VERILATOR_ADDED_COND_WIRE");
         modp->addStmtsp(asn);
        
        UINFO(6, "  [condtap] Marked variable with tag: " << lhsVar->name() << std::endl);
    }


    void addIf(AstIf* nodep) {
        UINFO(6, "Visiting If node at " << nodep->fileline()->ascii() << std::endl);
        FileLine* fl = nodep->fileline();
        UINFO(1, "Found IF at " << fl->ascii() << std::endl);

        AstNodeModule* modp = safeEnclosingModule(nodep);
        if (!modp) {
            UINFO(6, "  [condtap] No suitable insertion context; skipping" << std::endl);
            return;
        }
        
        if (!nodep->condp()) {
            UINFO(6, "  [condtap] No condition expr; skipping" << std::endl);
            return;
        }

        std::string name = makeName(fl, "Added__Vcond_if");
        AstVar* varp = declareWire(fl, modp, name);
        assignCondStoreWire(fl, modp, varp, nodep->condp());

        UINFO(6, "  [condtap] Inserted wire+assign: " << name << std::endl);
    }

    void addTernary(AstCond* nodep) {
        FileLine* fl = nodep->fileline();
        AstNodeModule* modp = safeEnclosingModule(nodep);
        // AstNode* insertPoint = findInsertionPoint(nodep);
        
        if (!modp  || !nodep->condp()) return;
        
        std::string name = makeName(fl, "Added__Vcond_tern");
        std::string qualifiedName = makeQualifiedName(modp, name);
        AstVar* varp = declareWire(fl, modp, name); 
        assignCondStoreWire(fl, modp, varp, nodep->condp());

        UINFO(6, "  [condtap] Inserted ternary cond wire: " << name << " @ " << fl->ascii() << std::endl);
    }

    void addCase(AstCase* nodep) {
        FileLine* fl = nodep->fileline();
        AstNodeModule* modp = safeEnclosingModule(nodep);
        // AstNode* insertPoint = findInsertionPoint(nodep);
        
        if (!modp || !nodep->exprp()) return;
        
        int idx = 0;
        for (AstNode* n = nodep->itemsp(); n; n = n->nextp()) {
            AstCaseItem* it = VN_AS(n, CaseItem);
            if (!it || !it->condsp()) continue;

            for (AstNode* e = it->condsp(); e; e = e->nextp()) {
                std::string base = makeName(fl, "Added__Vcond_case");
                std::string name = base + "_case" + cvtToStr(++idx);
                std::string qualifiedName = makeQualifiedName(modp, name);

                AstVar* varp = declareWire(fl, modp, name);
                AstNodeExpr* sel = nodep->exprp()->cloneTree(true);
                AstNodeExpr* val = VN_AS(e->cloneTree(true), NodeExpr);
                AstEq* cmp = new AstEq(fl, sel, val);
                assignCondStoreWire(fl, modp, varp, cmp);

                UINFO(6, "  [condtap] Inserted case flag: " << name << " @ " << fl->ascii() << std::endl);
            }
        }
    }

    void visit(AstIf* nodep) override      { addIf(nodep);      iterateChildren(nodep); }
    void visit(AstCond* nodep) override    { addTernary(nodep); iterateChildren(nodep); }
    void visit(AstCase* nodep) override    { addCase(nodep);    iterateChildren(nodep); }
    void visit(AstNode* nodep) override    { iterateChildren(nodep); }

public:
    explicit CondTraceVisitor(AstNetlist* rootp) {
        iterate(rootp);
    }
};
}  // namespace

void V3AddCondPass::run(AstNetlist* rootp) {
    UINFO(2, "V3AddCondPass::run\n");
    std::cout << "V3AddCondPass::run\n";
    if (!v3Global.opt.traceConditions()) {
        UINFO(3, "V3AddCondPass: --trace-conditions not enabled, skipping\n");
        return;
    }
    
    CondTraceVisitor visitor{rootp};
    V3Global::dumpCheckGlobalTree("condtrace", 0, dumpTreeEitherLevel() >= 3);
}