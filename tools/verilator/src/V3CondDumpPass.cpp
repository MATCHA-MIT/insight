#include "V3PchAstNoMT.h"
#include "V3CondDumpPass.h"

#include "V3Ast.h"
#include "V3Global.h"

#include <fstream>
#include <sstream>
#include <filesystem>

VL_DEFINE_DEBUG_FUNCTIONS;

namespace {

    // --- helpers: escape JSON strings and write quoted strings
    static void jsonEscapeAndQuote(std::ostream& os, const std::string& s) {
        os << '"';
        for (char c : s) {
            switch (c) {
            case '\\': os << "\\\\"; break;
            case '"' : os << "\\\""; break;
            case '\n': os << "\\n";  break;
            case '\r': os << "\\r";  break;
            case '\t': os << "\\t";  break;
            default:   os << c;      break;
            }
        }
        os << '"';
    }

    // Reuse your hierarchical name builder
    static std::string getFullHierName(AstVarRef* refp) {
        if (!refp) return "<null_ref>";
        if (AstVarScope* vscp = refp->varScopep()) {
            if (AstScope* scopep = vscp->scopep()) {
                std::string scopeName = scopep->prettyName();
                for (size_t pos = 0; (pos = scopeName.find("__", pos)) != std::string::npos; ++pos)
                    scopeName.replace(pos, 2, ".");
                return scopeName + "." + vscp->varp()->prettyName();
            }
        }
        return refp->prettyName();
    }

    // Serialize an expression to a JSON object (no outer quotes)
    static void exprToJson(AstNodeExpr* e, std::ostream& os);

    // Small helpers for emitting binary/unary JSON
    static void emitUnary(std::ostream& os, const char* op, AstNodeExpr* a) {
        os << "{\"type\":\"unary\",\"op\":";
        jsonEscapeAndQuote(os, op);
        os << ",\"arg\":";
        exprToJson(a, os);
        os << "}";
    }

    static void emitBinary(std::ostream& os, const char* op, AstNodeExpr* a, AstNodeExpr* b) {
        os << "{\"type\":\"binary\",\"op\":";
        jsonEscapeAndQuote(os, op);
        os << ",\"lhs\":";
        exprToJson(a, os);
        os << ",\"rhs\":";
        exprToJson(b, os);
        os << "}";
    }

    static void exprToJson(AstNodeExpr* e, std::ostream& os) {
        if (!e) { os << "null"; return; }

        if (AstVarRef* v = VN_CAST(e, VarRef)) {
            os << "{\"type\":\"var\",\"name\":";
            jsonEscapeAndQuote(os, getFullHierName(v));
            os << "}";
            return;
        }
        if (AstConst* c = VN_CAST(e, Const)) {
            os << "{\"type\":\"const\",\"value\":";
            jsonEscapeAndQuote(os, c->num().ascii());
            os << "}";
            return;
        }

        // Unary reductions
        if (AstRedOr* n = VN_CAST(e, RedOr))  { emitUnary(os, "redor",  n->lhsp()); return; }
        if (AstRedAnd* n = VN_CAST(e, RedAnd)){ emitUnary(os, "redand", n->lhsp()); return; }
        if (AstRedXor* n = VN_CAST(e, RedXor)){ emitUnary(os, "redxor", n->lhsp()); return; }

        // Logical / bitwise negations
        if (AstNot* n = VN_CAST(e, Not))      { emitUnary(os, "not",  n->lhsp()); return; }
        if (AstLogNot* n = VN_CAST(e, LogNot)){ emitUnary(os, "lnot", n->lhsp()); return; }

        // Equality
        if (AstEq* n = VN_CAST(e, Eq))        { emitBinary(os, "eq",   n->lhsp(), n->rhsp()); return; }
        if (AstNeq* n = VN_CAST(e, Neq))      { emitBinary(os, "ne",   n->lhsp(), n->rhsp()); return; }
        if (AstEqCase* n = VN_CAST(e, EqCase)){ emitBinary(os, "eqx",  n->lhsp(), n->rhsp()); return; }
        if (AstNeqCase* n = VN_CAST(e, NeqCase)){ emitBinary(os, "nex", n->lhsp(), n->rhsp()); return; }

        // Comparison
        if (AstLt* n = VN_CAST(e, Lt))        { emitBinary(os, "lt",   n->lhsp(), n->rhsp()); return; }
        if (AstLte* n = VN_CAST(e, Lte))      { emitBinary(os, "lte",  n->lhsp(), n->rhsp()); return; }
        if (AstGt* n = VN_CAST(e, Gt))        { emitBinary(os, "gt",   n->lhsp(), n->rhsp()); return; }
        if (AstGte* n = VN_CAST(e, Gte))      { emitBinary(os, "gte",  n->lhsp(), n->rhsp()); return; }

        // Logical/bitwise operators
        if (AstAnd* n = VN_CAST(e, And))      { emitBinary(os, "and",  n->lhsp(), n->rhsp()); return; }
        if (AstOr* n = VN_CAST(e, Or))        { emitBinary(os, "or",   n->lhsp(), n->rhsp()); return; }
        if (AstXor* n = VN_CAST(e, Xor))      { emitBinary(os, "xor",  n->lhsp(), n->rhsp()); return; }
        if (AstLogAnd* n = VN_CAST(e, LogAnd)){ emitBinary(os, "land", n->lhsp(), n->rhsp()); return; }
        if (AstLogOr* n = VN_CAST(e, LogOr))  { emitBinary(os, "lor",  n->lhsp(), n->rhsp()); return; }

        // Arithmetic
        if (AstAdd* n = VN_CAST(e, Add))      { emitBinary(os, "add",  n->lhsp(), n->rhsp()); return; }
        if (AstSub* n = VN_CAST(e, Sub))      { emitBinary(os, "sub",  n->lhsp(), n->rhsp()); return; }
        if (AstMul* n = VN_CAST(e, Mul))      { emitBinary(os, "mul",  n->lhsp(), n->rhsp()); return; }
        if (AstDiv* n = VN_CAST(e, Div))      { emitBinary(os, "div",  n->lhsp(), n->rhsp()); return; }
        if (AstModDiv* n = VN_CAST(e, ModDiv)){ emitBinary(os, "mod",  n->lhsp(), n->rhsp()); return; }

        // Shifts
        if (AstShiftL* n = VN_CAST(e, ShiftL)){ emitBinary(os, "shl",  n->lhsp(), n->rhsp()); return; }
        if (AstShiftR* n = VN_CAST(e, ShiftR)){ emitBinary(os, "shr",  n->lhsp(), n->rhsp()); return; }
        if (AstShiftRS* n = VN_CAST(e, ShiftRS)){ emitBinary(os, "ashr", n->lhsp(), n->rhsp()); return; }

        // Slice, indexing, replication
        if (AstSel* n = VN_CAST(e, Sel)) {
            os << "{\"type\":\"slice\",\"value\":";
            exprToJson(n->fromp(), os);
            os << ",\"lsb\":";
            exprToJson(n->lsbp(), os);
            if (n->widthConst() > 0) os << ",\"width\":" << n->widthConst();
            os << "}";
            return;
        }
        if (AstArraySel* n = VN_CAST(e, ArraySel)) {
            os << "{\"type\":\"index\",\"value\":";
            exprToJson(n->fromp(), os);
            os << ",\"index\":";
            exprToJson(n->bitp(), os);
            os << "}";
            return;
        }

        // Conditional (mux)
        if (AstCond* n = VN_CAST(e, Cond)) {
            os << "{\"type\":\"mux\",\"cond\":";
            exprToJson(n->condp(), os);
            os << ",\"then\":";
            exprToJson(n->thenp(), os);
            os << ",\"else\":";
            exprToJson(n->elsep(), os);
            os << "}";
            return;
        }

        // Concatenation
        if (AstConcat* n = VN_CAST(e, Concat)) {
            os << "{\"type\":\"concat\",\"lhs\":";
            exprToJson(n->lhsp(), os);
            os << ",\"rhs\":";
            exprToJson(n->rhsp(), os);
            os << "}";
            return;
        }

        // Replication
        if (AstReplicate* n = VN_CAST(e, Replicate)) {
            os << "{\"type\":\"replicate\",\"count\":";
            exprToJson(n->countp(), os);
            os << ",\"value\":";
            exprToJson(n->srcp(), os);
            os << "}";
            return;
        }

        // Fallback for unknown expression kinds
        os << "{\"type\":\"unknown\",\"node\":";
        jsonEscapeAndQuote(os, e->typeName());
        os << "}";
    }



class CondDumpVisitor final : public VNVisitor {
    /*
    wireToExpr should map each conditional wire to a json representation of the expression.
    */
    std::map<std::string, std::string> m_wireToExpr;

    // Get fully qualified hierarchical name from a VarRef after scoping
    static std::string getFullHierName(AstVarRef* refp) {
        if (!refp) return "<null_ref>"; 
        
        // After V3Scope, we should have AstVarScope available
        if (AstVarScope* vscp = refp->varScopep()) {
            if (AstScope* scopep = vscp->scopep()) {
                std::string scopeName = scopep->prettyName();
                // Replace __ with . for readability
                size_t pos = 0;
                while ((pos = scopeName.find("__", pos)) != std::string::npos) {
                    scopeName.replace(pos, 2, ".");
                    pos += 1;
                }
                return scopeName + "." + vscp->varp()->prettyName();
            }
        }
        
        // Fallback
        return refp->prettyName();
    }

    static void printExpr(std::ostream& os, AstNodeExpr* exprp) {
        if (!exprp) { os << "<null>"; return; }

        if (AstVarRef* ref = VN_CAST(exprp, VarRef)) {
            os << getFullHierName(ref);
        } else if (AstConst* cst = VN_CAST(exprp, Const)) {
            os << cst->num().ascii();
        } else if (AstRedOr* redOrp = VN_CAST(exprp, RedOr)) {
            os << "|";
            printExpr(os, redOrp->lhsp());
        } else if (AstRedAnd* redAndp = VN_CAST(exprp, RedAnd)) {
            os << "&";
            printExpr(os, redAndp->lhsp());
        } else if (AstRedXor* redXorp = VN_CAST(exprp, RedXor)) {
            os << "^";
            printExpr(os, redXorp->lhsp());
        } else if (AstNot* notp = VN_CAST(exprp, Not)) {
            os << "~";
            printExpr(os, notp->lhsp());
        } else if (AstLogNot* logNotp = VN_CAST(exprp, LogNot)) {
            os << "!";
            printExpr(os, logNotp->lhsp());
        } else if (AstEq* eq = VN_CAST(exprp, Eq)) {
            printExpr(os, eq->lhsp());
            os << " == ";
            printExpr(os, eq->rhsp());
        } else if (AstNeq* neq = VN_CAST(exprp, Neq)) {
            printExpr(os, neq->lhsp());
            os << " != ";
            printExpr(os, neq->rhsp());
        } else if (AstEqCase* eqCase = VN_CAST(exprp, EqCase)) {
            printExpr(os, eqCase->lhsp());
            os << " === ";
            printExpr(os, eqCase->rhsp());
        } else if (AstNeqCase* neqCase = VN_CAST(exprp, NeqCase)) {
            printExpr(os, neqCase->lhsp());
            os << " !== ";
            printExpr(os, neqCase->rhsp());
        } else if (AstLt* lt = VN_CAST(exprp, Lt)) {
            printExpr(os, lt->lhsp());
            os << " < ";
            printExpr(os, lt->rhsp());
        } else if (AstLte* lte = VN_CAST(exprp, Lte)) {
            printExpr(os, lte->lhsp());
            os << " <= ";
            printExpr(os, lte->rhsp());
        } else if (AstGt* gt = VN_CAST(exprp, Gt)) {
            printExpr(os, gt->lhsp());
            os << " > ";
            printExpr(os, gt->rhsp());
        } else if (AstGte* gte = VN_CAST(exprp, Gte)) {
            printExpr(os, gte->lhsp());
            os << " >= ";
            printExpr(os, gte->rhsp());
        } else if (AstAnd* andp = VN_CAST(exprp, And)) {
            os << "(";
            printExpr(os, andp->lhsp());
            os << " & ";
            printExpr(os, andp->rhsp());
            os << ")";
        } else if (AstOr* orp = VN_CAST(exprp, Or)) {
            os << "(";
            printExpr(os, orp->lhsp());
            os << " | ";
            printExpr(os, orp->rhsp());
            os << ")";
        } else if (AstXor* xorp = VN_CAST(exprp, Xor)) {
            os << "(";
            printExpr(os, xorp->lhsp());
            os << " ^ ";
            printExpr(os, xorp->rhsp());
            os << ")";
        } else if (AstLogAnd* logAndp = VN_CAST(exprp, LogAnd)) {
            os << "(";
            printExpr(os, logAndp->lhsp());
            os << " && ";
            printExpr(os, logAndp->rhsp());
            os << ")";
        } else if (AstLogOr* logOrp = VN_CAST(exprp, LogOr)) {
            os << "(";
            printExpr(os, logOrp->lhsp());
            os << " || ";
            printExpr(os, logOrp->rhsp());
            os << ")";
        } else if (AstSel* selp = VN_CAST(exprp, Sel)) {
            printExpr(os, selp->fromp());
            os << "[";
            printExpr(os, selp->lsbp());
            if (selp->widthConst() > 1) {
                os << " +: ";
                os << selp->widthConst();
            }
            os << "]";
        } else if (AstArraySel* arrSelp = VN_CAST(exprp, ArraySel)) {
            printExpr(os, arrSelp->fromp());
            os << "[";
            printExpr(os, arrSelp->bitp());
            os << "]";
        } else if (AstCond* condp = VN_CAST(exprp, Cond)) {
            os << "(";
            printExpr(os, condp->condp());
            os << " ? ";
            printExpr(os, condp->thenp());
            os << " : ";
            printExpr(os, condp->elsep());
            os << ")";
        } else if (AstConcat* concatp = VN_CAST(exprp, Concat)) {
            os << "{";
            printExpr(os, concatp->lhsp());
            os << ", ";
            printExpr(os, concatp->rhsp());
            os << "}";
        } else if (AstReplicate* repp = VN_CAST(exprp, Replicate)) {
            os << "{";
            printExpr(os, repp->countp());  // Print count first (matches Verilog syntax)
            os << "{";
            printExpr(os, repp->srcp());    // Then print the source expression
            os << "}}";
        } else if (AstAdd* addp = VN_CAST(exprp, Add)) {
            os << "(";
            printExpr(os, addp->lhsp());
            os << " + ";
            printExpr(os, addp->rhsp());
            os << ")";
        } else if (AstSub* subp = VN_CAST(exprp, Sub)) {
            os << "(";
            printExpr(os, subp->lhsp());
            os << " - ";
            printExpr(os, subp->rhsp());
            os << ")";
        } else if (AstMul* mulp = VN_CAST(exprp, Mul)) {
            os << "(";
            printExpr(os, mulp->lhsp());
            os << " * ";
            printExpr(os, mulp->rhsp());
            os << ")";
        } else if (AstDiv* divp = VN_CAST(exprp, Div)) {
            os << "(";
            printExpr(os, divp->lhsp());
            os << " / ";
            printExpr(os, divp->rhsp());
            os << ")";
        } else if (AstModDiv* modp = VN_CAST(exprp, ModDiv)) {
            os << "(";
            printExpr(os, modp->lhsp());
            os << " % ";
            printExpr(os, modp->rhsp());
            os << ")";
        } else if (AstShiftL* shlp = VN_CAST(exprp, ShiftL)) {
            os << "(";
            printExpr(os, shlp->lhsp());
            os << " << ";
            printExpr(os, shlp->rhsp());
            os << ")";
        } else if (AstShiftR* shrp = VN_CAST(exprp, ShiftR)) {
            os << "(";
            printExpr(os, shrp->lhsp());
            os << " >> ";
            printExpr(os, shrp->rhsp());
            os << ")";
        } else if (AstShiftRS* shrsp = VN_CAST(exprp, ShiftRS)) {
            os << "(";
            printExpr(os, shrsp->lhsp());
            os << " >>> ";
            printExpr(os, shrsp->rhsp());
            os << ")";
        } else {
            // Fallback for unhandled expression types
            os << "<" << exprp->typeName() << ">";
        }
    }

    static std::string exprToString(AstNodeExpr* exprp) {
        std::ostringstream os;
        printExpr(os, exprp);
        return os.str();
    }

    // void processAttr(AstAttrOf* attrp) {
    //     // Check if this is our marker attribute
    //     if (AstText* textp = VN_CAST(attrp->fromp(), Text)) {
    //         const std::string text = textp->text();
    //         if (text.find("verilator_cond_wire:") == 0) {
    //             // Extract wire name from attribute
    //             //const std::string wireName = text.substr(20);  // Skip "verilator_cond_wire:"
                
    //             // Get the expression this attribute is attached to
    //             AstNode* parentp = attrp->backp();
    //             if (AstNodeExpr* exprp = VN_CAST(parentp, NodeExpr)) {
    //                 std::string exprStr = exprToString(exprp);
    //                 std::string wireName = parentp->prettyName();
    //                 // Try to find the scope to build qualified name
    //                 std::string qualifiedName = "";
    //                 for (AstNode* p = attrp; p; p = p->backp()) {
    //                     if (AstScope* scopep = VN_CAST(p, Scope)) {
    //                         std::string scopeName = scopep->prettyName();
    //                         size_t pos = 0;
    //                         while ((pos = scopeName.find("__", pos)) != std::string::npos) {
    //                             scopeName.replace(pos, 2, ".");
    //                             pos += 1;
    //                         }
    //                         qualifiedName = scopeName + "." + wireName;
    //                         break;
    //                     }
    //                 }
    //                 std::ostringstream os;
    //                 assignp->rhsp()->dumpJson(os);
    //                 m_wireToExpr[qualifiedName] = os.str();
    //                 std::cout << "processAttr Recorded condition for " << qualifiedName 
    //                          << ": " << os.str() << std::endl;
    //             }
    //         }
    //     }
    // }

    void writeJsonOutput() {
        const std::string jsonFile = v3Global.opt.makeDir() + "/cond_map.json";
        #include <filesystem>

        std::string dir = v3Global.opt.makeDir();
        if (!dir.empty()) {
            namespace fs = std::filesystem;
            std::error_code ec;
            if (!fs::exists(dir)) {
                if (!fs::create_directories(dir, ec)) {
                    v3warn(EC_ERROR, "Cannot create directory " << dir << ": " << ec.message());
                } else {
                    UINFO(3, "Created directory " << dir << std::endl);
                }
            }
        }
        UINFO(2, "Writing condition mapping to " << jsonFile << std::endl);
        std::ofstream json(jsonFile);
        if (!json.is_open()) {
            v3warn(EC_ERROR, "Cannot write condition map to " << jsonFile);
            return;
        }

        json << "{\n";
        bool first = true;
        for (const auto& kv : m_wireToExpr) {
            if (!first) json << ",\n";
            first = false;
            json << "  ";
            jsonEscapeAndQuote(json, kv.first);
            json << ": " << kv.second;  // already a JSON object
        }
        json << "\n}\n";
        json.close();
        
        UINFO(2, "Wrote condition mapping to " << jsonFile << std::endl);
    }


    // void processVar(AstVar* varp) {
    //     // Check if this variable has our marker tag
    //     if (varp->tag() == "VERILATOR_COND_WIRE") {
    //         std::cout << "Found condition wire: " << varp->name() << std::endl;
    //     }
    // }

    void processAssign(AstAssignW* assignp) {
        // Get the variable reference on the LHS
        AstVarRef* lhsp = VN_CAST(assignp->lhsp(), VarRef);
        if (!lhsp) return;
        
        // Get the actual variable
        AstVar* varp = lhsp->varp();
        if (!varp) return;
        
        // Check the tag on the VARIABLE (not the assignment)
        if (varp->tag() != "VERILATOR_ADDED_COND_WIRE") return;
        
        UINFO(6, "Found assignment to condition wire: " << varp->name() << " at "
                     << assignp->fileline()->ascii() << std::endl);
        UINFO(6, "LHS: " << assignp->lhsp()->prettyName() << std::endl);
        // std::ostringstream os;
        // assignp->rhsp()->dumpJson(os);
        // UINFO(6, "RHS JSON: " << os.str() << std::endl);
 
        // Extract the condition expression from the RHS
        std::ostringstream os;
        exprToJson(assignp->rhsp(), os);
        std::string exprStr = os.str();
        
        // Build qualified name using scope information
        std::string qualifiedName = varp->prettyName();
        for (AstNode* p = assignp; p; p = p->backp()) {
            if (AstScope* scopep = VN_CAST(p, Scope)) {
                std::string scopeName = scopep->prettyName();
                size_t pos = 0;
                while ((pos = scopeName.find("__", pos)) != std::string::npos) {
                    scopeName.replace(pos, 2, ".");
                    pos += 1;
                }
                qualifiedName = scopeName + "." + varp->prettyName();
                break;
            }
        }
        
        m_wireToExpr[qualifiedName] = exprStr;
        UINFO(6, "Recorded condition for " << qualifiedName << ": " << exprStr << std::endl);
    }

    // void visit(AstAttrOf* nodep) override {
    //     processAttr(nodep);
    //     iterateChildren(nodep);
    // }

    // void visit(AstVar* nodep) override {
    //     processVar(nodep);
    //     iterateChildren(nodep);
    // }

    void visit(AstAssignW* nodep) override {
        processAssign(nodep);
        iterateChildren(nodep);
    }
    void visit(AstNode* nodep) override { iterateChildren(nodep); }

public:
    explicit CondDumpVisitor(AstNetlist* rootp) {
        iterate(rootp);
        writeJsonOutput();
    }
};
}  // namespace

void V3CondDumpPass::run(AstNetlist* rootp) {
    UINFO(2, "V3CondDumpPass::run\n");
    UINFO(3, "V3CondDumpPass: (debug) starting run\n");
    
    if (!v3Global.opt.traceConditions()) {
        UINFO(3, "V3CondDumpPass: --trace-conditions not enabled, skipping\n");
        return;       
    }
    
    CondDumpVisitor visitor{rootp};
    V3Global::dumpCheckGlobalTree("conddump", 0, dumpTreeEitherLevel() >= 3);
}
