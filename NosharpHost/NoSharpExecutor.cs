using System.Diagnostics;
using System.Globalization;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace NosharpHost {
    public enum OpCode : byte {
        LoadConst,
        LoadLocal,
        StoreLocal,
        Add,
        Sub,
        Mul,
        Div,
        Mod,
        Negate,
        Equal,
        NotEqual,
        LessThan,
        LessThanOrEqual,
        GreaterThan,
        GreaterThanOrEqual,
        And,
        Or,
        Not,
        Jump,
        JumpIfFalse,
        Call,
        Return,
        StartTimer,
        StopTimer,
        Print,
        Pop,
        JumpIfTrue,
        ShortCircuitAnd,
        ShortCircuitOr
    }

    [StructLayout(LayoutKind.Sequential, Pack = 4)]
    public readonly struct Instruction {
        public readonly OpCode Op;
        public readonly int Operand;

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public Instruction(OpCode op, int operand = 0) {
            Op = op;
            Operand = operand;
        }
    }

    public enum ValueType : byte {
        Null,
        Number,
        Boolean,
        String
    }

    public readonly struct NoValue : IEquatable<NoValue> {
        public readonly ValueType Type;
        public readonly double NumberValue;
        public readonly bool BoolValue;
        public readonly string StringValue;

        public static NoValue Null => new NoValue(ValueType.Null, 0, false, "null");
        public static NoValue True => new NoValue(true);
        public static NoValue False => new NoValue(false);
        public static NoValue Zero => new NoValue(0.0);
        public static NoValue One => new NoValue(1.0);

        public NoValue(double val) {
            Type = ValueType.Number;
            NumberValue = val;
            BoolValue = false;
            StringValue = string.Empty;
        }

        public NoValue(bool val) {
            Type = ValueType.Boolean;
            NumberValue = 0;
            BoolValue = val;
            StringValue = string.Empty;
        }

        public NoValue(string val) {
            Type = ValueType.String;
            NumberValue = 0;
            BoolValue = false;
            StringValue = val ?? string.Empty;
        }

        private NoValue(ValueType type, double num, bool b, string s) {
            Type = type;
            NumberValue = num;
            BoolValue = b;
            StringValue = s ?? string.Empty;
        }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public bool IsTruthy() {
            return Type switch {
                ValueType.Boolean => BoolValue,
                ValueType.Number => NumberValue != 0 && !double.IsNaN(NumberValue),
                ValueType.String => StringValue.Length > 0,
                _ => false
            };
        }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public bool Equals(NoValue other) {
            if (Type != other.Type) return false;
            return Type switch {
                ValueType.Number => NumberValue.Equals(other.NumberValue),
                ValueType.Boolean => BoolValue == other.BoolValue,
                ValueType.String => StringValue == other.StringValue,
                ValueType.Null => true,
                _ => false
            };
        }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public override bool Equals(object? obj) => obj is NoValue other && Equals(other);

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public override int GetHashCode() {
            return Type switch {
                ValueType.Number => HashCode.Combine(Type, NumberValue),
                ValueType.Boolean => HashCode.Combine(Type, BoolValue),
                ValueType.String => StringValue.GetHashCode(),
                _ => (int)Type
            };
        }

        public override string ToString() {
            return Type switch {
                ValueType.Number => NumberValue.ToString(CultureInfo.InvariantCulture),
                ValueType.Boolean => BoolValue ? "true" : "false",
                ValueType.String => StringValue,
                ValueType.Null => "null",
                _ => "null"
            };
        }
    }

    public class CompiledFunction {
        public string Name { get; set; } = string.Empty;
        public int ParamCount { get; set; }
        public int LocalCount { get; set; }
        public Instruction[] Code { get; set; } = Array.Empty<Instruction>();
    }

    public class CompiledProgram {
        public Instruction[] MainCode { get; set; } = Array.Empty<Instruction>();
        public NoValue[] Constants { get; set; } = Array.Empty<NoValue>();
        public CompiledFunction[] FunctionsArray { get; set; } = Array.Empty<CompiledFunction>();
        public int MainLocalCount { get; set; }
    }

    public struct CallFrame {
        public Instruction[] Code;
        public int IP;
        public int StackBase;
        public int LocalBase;
    }

    public class BytecodeCompiler {
        private readonly Dictionary<string, CompiledFunction> functionsByName = new(StringComparer.Ordinal);
        private readonly Dictionary<string, int> functionIndexesByName = new(StringComparer.Ordinal);
        private readonly List<CompiledFunction> funcList = new();
        private readonly List<NoValue> constants = new();
        private readonly Dictionary<NoValue, int> constantCache = new();

        private List<Instruction> currentCode = new();
        private Dictionary<string, int> currentLocals = new(StringComparer.Ordinal);

        public CompiledProgram Compile(JsonElement root) {
            functionsByName.Clear();
            functionIndexesByName.Clear();
            funcList.Clear();
            constants.Clear();
            constantCache.Clear();

            foreach (JsonElement cmd in root.EnumerateArray()) {
                string? type = cmd.GetProperty("type").GetString();
                if (type == "DefineFunction") {
                    string name = cmd.GetProperty("name").GetString()!;
                    int paramCount = cmd.GetProperty("params").GetArrayLength();

                    var func = new CompiledFunction {
                        Name = name,
                        ParamCount = paramCount
                    };

                    functionsByName[name] = func;
                    functionIndexesByName[name] = funcList.Count;
                    funcList.Add(func);
                }
            }

            foreach (JsonElement cmd in root.EnumerateArray()) {
                string? type = cmd.GetProperty("type").GetString();
                if (type == "DefineFunction") {
                    CompileFunctionDef(cmd);
                }
            }

            currentCode = new List<Instruction>(256);
            currentLocals.Clear();

            foreach (JsonElement cmd in root.EnumerateArray()) {
                string? type = cmd.GetProperty("type").GetString();
                if (type != "DefineFunction") {
                    CompileCommand(cmd);
                }
            }

            currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.Null)));
            currentCode.Add(new Instruction(OpCode.Return));

            return new CompiledProgram {
                MainCode = currentCode.ToArray(),
                Constants = constants.ToArray(),
                FunctionsArray = funcList.ToArray(),
                MainLocalCount = currentLocals.Count
            };
        }

        private void CompileFunctionDef(JsonElement cmd) {
            string funcName = cmd.GetProperty("name").GetString()!;
            CompiledFunction func = functionsByName[funcName];

            var outerCode = currentCode;
            var outerLocals = currentLocals;

            currentCode = new List<Instruction>(128);
            currentLocals = new Dictionary<string, int>(StringComparer.Ordinal);

            JsonElement paramsArr = cmd.GetProperty("params");
            foreach (JsonElement param in paramsArr.EnumerateArray()) {
                string paramName = param.ValueKind == JsonValueKind.Array ? param[1].GetString()! : param.GetString()!;
                GetOrAddLocal(paramName);
            }

            JsonElement bodyArr = cmd.GetProperty("body");
            foreach (JsonElement bodyCmd in bodyArr.EnumerateArray()) {
                CompileCommand(bodyCmd);
            }

            currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.Null)));
            currentCode.Add(new Instruction(OpCode.Return));

            func.Code = currentCode.ToArray();
            func.LocalCount = currentLocals.Count;

            currentCode = outerCode;
            currentLocals = outerLocals;
        }

        private void CompileCommand(JsonElement cmd) {
            string? type = cmd.GetProperty("type").GetString();

            switch (type) {
                case "SetVar":
                case "AssignVar": {
                    string varName = cmd.GetProperty("name").GetString()!;
                    int localIdx = GetOrAddLocal(varName);

                    if (cmd.TryGetProperty("value", out JsonElement valExpr) && valExpr.ValueKind != JsonValueKind.Null) {
                        CompileExpr(valExpr);
                    } else {
                        currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.Null)));
                    }

                    currentCode.Add(new Instruction(OpCode.StoreLocal, localIdx));
                    break;
                }

                case "If": {
                    CompileExpr(cmd.GetProperty("condition"));

                    int jumpIfFalseIdx = currentCode.Count;
                    currentCode.Add(new Instruction(OpCode.JumpIfFalse, 0));

                    if (cmd.TryGetProperty("then_block", out JsonElement thenArr) && thenArr.ValueKind == JsonValueKind.Array) {
                        foreach (JsonElement c in thenArr.EnumerateArray()) {
                            CompileCommand(c);
                        }
                    }

                    bool hasElse = cmd.TryGetProperty("else_block", out JsonElement elseArr) &&
                                  elseArr.ValueKind == JsonValueKind.Array &&
                                  elseArr.GetArrayLength() > 0;

                    if (hasElse) {
                        int jumpEndIdx = currentCode.Count;
                        currentCode.Add(new Instruction(OpCode.Jump, 0));

                        int elseStart = currentCode.Count;
                        currentCode[jumpIfFalseIdx] = new Instruction(OpCode.JumpIfFalse, elseStart);

                        foreach (JsonElement c in elseArr.EnumerateArray()) {
                            CompileCommand(c);
                        }

                        int endStart = currentCode.Count;
                        currentCode[jumpEndIdx] = new Instruction(OpCode.Jump, endStart);
                    } else {
                        int endStart = currentCode.Count;
                        currentCode[jumpIfFalseIdx] = new Instruction(OpCode.JumpIfFalse, endStart);
                    }

                    break;
                }

                case "For": {
                    string varName = cmd.GetProperty("name").GetString()!;
                    int iterSlot = GetOrAddLocal(varName);

                    CompileExpr(cmd.GetProperty("start"));
                    currentCode.Add(new Instruction(OpCode.StoreLocal, iterSlot));

                    int loopStartIdx = currentCode.Count;

                    currentCode.Add(new Instruction(OpCode.LoadLocal, iterSlot));
                    CompileExpr(cmd.GetProperty("end"));
                    currentCode.Add(new Instruction(OpCode.LessThan));

                    int jumpOutIdx = currentCode.Count;
                    currentCode.Add(new Instruction(OpCode.JumpIfFalse, 0));

                    if (cmd.TryGetProperty("body", out JsonElement bodyArr) && bodyArr.ValueKind == JsonValueKind.Array) {
                        foreach (JsonElement c in bodyArr.EnumerateArray()) {
                            CompileCommand(c);
                        }
                    }

                    currentCode.Add(new Instruction(OpCode.LoadLocal, iterSlot));
                    if (cmd.TryGetProperty("step", out JsonElement stepExpr) && stepExpr.ValueKind != JsonValueKind.Null) {
                        CompileExpr(stepExpr);
                    } else {
                        currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.One)));
                    }

                    currentCode.Add(new Instruction(OpCode.Add));
                    currentCode.Add(new Instruction(OpCode.StoreLocal, iterSlot));

                    currentCode.Add(new Instruction(OpCode.Jump, loopStartIdx));

                    int loopEndIdx = currentCode.Count;
                    currentCode[jumpOutIdx] = new Instruction(OpCode.JumpIfFalse, loopEndIdx);
                    break;
                }

                case "CallFunction": {
                    string funcName = cmd.GetProperty("name").GetString()!;
                    JsonElement? argsArr = cmd.TryGetProperty("args", out JsonElement a) ? a : null;
                    CompileCall(funcName, argsArr, isStatement: true);
                    break;
                }

                case "Return": {
                    if (cmd.TryGetProperty("value", out JsonElement retVal) && retVal.ValueKind != JsonValueKind.Null) {
                        CompileExpr(retVal);
                    } else {
                        currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.Null)));
                    }

                    currentCode.Add(new Instruction(OpCode.Return));
                    break;
                }
            }
        }

        private void CompileExpr(JsonElement expr) {
            string? type = expr.GetProperty("type").GetString();

            switch (type) {
                case "Literal": {
                    JsonElement val = expr.GetProperty("value");
                    if (val.ValueKind == JsonValueKind.Number) {
                        currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(new NoValue(val.GetDouble()))));
                    } else if (val.ValueKind == JsonValueKind.True || val.ValueKind == JsonValueKind.False) {
                        currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(new NoValue(val.GetBoolean()))));
                    } else if (val.ValueKind == JsonValueKind.String) {
                        currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(new NoValue(val.GetString()!))));
                    } else {
                        currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.Null)));
                    }

                    break;
                }

                case "Variable": {
                    string name = expr.GetProperty("name").GetString()!;
                    int localIdx = GetOrAddLocal(name);
                    currentCode.Add(new Instruction(OpCode.LoadLocal, localIdx));
                    break;
                }

                case "BinaryOp": {
                    string op = expr.GetProperty("op").GetString()!;
                    
                    // Short-circuit evaluation for logical ops
                    if (op == "and") {
                        CompileExpr(expr.GetProperty("left"));
                        int jumpIdx = currentCode.Count;
                        currentCode.Add(new Instruction(OpCode.ShortCircuitAnd, 0));
                        currentCode.Add(new Instruction(OpCode.Pop));
                        
                        CompileExpr(expr.GetProperty("right"));
                        currentCode[jumpIdx] = new Instruction(OpCode.ShortCircuitAnd, currentCode.Count);
                        break;
                    }
                    
                    if (op == "or") {
                        CompileExpr(expr.GetProperty("left"));
                        int jumpIdx = currentCode.Count;
                        currentCode.Add(new Instruction(OpCode.ShortCircuitOr, 0));
                        currentCode.Add(new Instruction(OpCode.Pop));
                        
                        CompileExpr(expr.GetProperty("right"));
                        currentCode[jumpIdx] = new Instruction(OpCode.ShortCircuitOr, currentCode.Count);
                        break;
                    }

                    CompileExpr(expr.GetProperty("left"));
                    CompileExpr(expr.GetProperty("right"));

                    switch (op) {
                        case "+": currentCode.Add(new Instruction(OpCode.Add)); break;
                        case "-": currentCode.Add(new Instruction(OpCode.Sub)); break;
                        case "*": currentCode.Add(new Instruction(OpCode.Mul)); break;
                        case "/": currentCode.Add(new Instruction(OpCode.Div)); break;
                        case "%": currentCode.Add(new Instruction(OpCode.Mod)); break;
                        case "==": currentCode.Add(new Instruction(OpCode.Equal)); break;
                        case "!=": currentCode.Add(new Instruction(OpCode.NotEqual)); break;
                        case "<": currentCode.Add(new Instruction(OpCode.LessThan)); break;
                        case "<=": currentCode.Add(new Instruction(OpCode.LessThanOrEqual)); break;
                        case ">": currentCode.Add(new Instruction(OpCode.GreaterThan)); break;
                        case ">=": currentCode.Add(new Instruction(OpCode.GreaterThanOrEqual)); break;
                        case "..": currentCode.Add(new Instruction(OpCode.Add)); break;
                    }
                    break;
                }

                case "UnaryOp": {
                    CompileExpr(expr.GetProperty("operand"));
                    string op = expr.GetProperty("op").GetString()!;
                    if (op == "not") {
                        currentCode.Add(new Instruction(OpCode.Not));
                    } else if (op == "-") {
                        currentCode.Add(new Instruction(OpCode.Negate));
                    }

                    break;
                }

                case "Call": {
                    string funcName = expr.GetProperty("name").GetString()!;
                    JsonElement? argsArr = expr.TryGetProperty("args", out JsonElement a) ? a : null;
                    CompileCall(funcName, argsArr, isStatement: false);
                    break;
                }
            }
        }

        private void CompileCall(string funcName, JsonElement? argsArr, bool isStatement) {
            if (funcName == "startTimer") {
                currentCode.Add(new Instruction(OpCode.StartTimer));
                if (!isStatement) currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.Null)));
            } else if (funcName == "stopTimer") {
                currentCode.Add(new Instruction(OpCode.StopTimer));
                if (!isStatement) currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.Null)));
            } else if (funcName == "print") {
                if (argsArr.HasValue && argsArr.Value.ValueKind == JsonValueKind.Array && argsArr.Value.GetArrayLength() > 0) {
                    CompileExpr(argsArr.Value[0]);
                } else {
                    currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.Null)));
                }

                currentCode.Add(new Instruction(OpCode.Print));
                if (!isStatement) currentCode.Add(new Instruction(OpCode.LoadConst, AddConstant(NoValue.Null)));
            } else {
                if (!functionsByName.TryGetValue(funcName, out CompiledFunction? targetFunc)) {
                    throw new InvalidOperationException($"[Compiler Error] Undefined function '{funcName}'.");
                }

                int funcIndex = functionIndexesByName[funcName];
                if (argsArr.HasValue && argsArr.Value.ValueKind == JsonValueKind.Array) {
                    foreach (JsonElement arg in argsArr.Value.EnumerateArray()) {
                        CompileExpr(arg);
                    }
                }

                currentCode.Add(new Instruction(OpCode.Call, funcIndex));
                if (isStatement) {
                    currentCode.Add(new Instruction(OpCode.Pop));
                }
            }
        }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private int GetOrAddLocal(string name) {
            if (currentLocals.TryGetValue(name, out int idx)) {
                return idx;
            }
            int newIdx = currentLocals.Count;
            currentLocals[name] = newIdx;
            return newIdx;
        }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private int AddConstant(NoValue val) {
            if (constantCache.TryGetValue(val, out int existingIndex)) {
                return existingIndex;
            }
            int newIndex = constants.Count;
            constants.Add(val);
            constantCache[val] = newIndex;
            return newIndex;
        }
    }

    public class VM {
        private const int MaxStackSize = 65536;
        private const int MaxLocalsSize = 131072;
        private const int MaxFramesSize = 4096;

        private readonly NoValue[] stack;
        private readonly NoValue[] locals;
        private readonly CallFrame[] frames;

        private Stopwatch? timer;

        public VM() {
            stack = new NoValue[MaxStackSize];
            locals = new NoValue[MaxLocalsSize];
            frames = new CallFrame[MaxFramesSize];
        }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private void ThrowStackOverflow(string msg) => throw new StackOverflowException(msg);

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        private void ThrowIndexOutOfRange(string msg) => throw new IndexOutOfRangeException(msg);

        public static Dictionary<string, object> GetNoSharpGlobalDictionary() {
            return new Dictionary<string, object> {
                ["print"] = new {parameters = new[] {"any"}, returns = "void"},
                ["startTimer"] = new {parameters = Array.Empty<string>(), returns = "void"},
                ["stopTimer"] = new {parameters = Array.Empty<string>(), returns = "void"}
            };
        }

        public void Execute(CompiledProgram program) {
            int sp = 0;
            int frameCount = 0;
            int localBase = 0;
            int nextLocalSlot = program.MainLocalCount;
            int ip = 0;

            Instruction[] code = program.MainCode;
            CompiledFunction[] functions = program.FunctionsArray;
            NoValue[] constants = program.Constants;
            int codeLength = code.Length;

            Array.Clear(locals, 0, program.MainLocalCount);
            while (ip < codeLength) {
                Instruction inst = code[ip++];

                switch (inst.Op) {
                    case OpCode.LoadConst:
                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on LoadConst");
                        stack[sp++] = constants[inst.Operand];
                        break;

                    case OpCode.LoadLocal:
                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on LoadLocal");
                        stack[sp++] = locals[localBase + inst.Operand];
                        break;

                    case OpCode.StoreLocal:
                        locals[localBase + inst.Operand] = stack[--sp];
                        break;

                    case OpCode.Add: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];

                        if (a.Type == ValueType.String || b.Type == ValueType.String) {
                            string left = a.Type == ValueType.String
                                ? a.StringValue
                                : a.ToString();

                            string right = b.Type == ValueType.String
                                ? b.StringValue
                                : b.ToString();

                            stack[sp++] = new NoValue(string.Concat(left, right));
                        } else {
                            stack[sp++] = new NoValue(a.NumberValue + b.NumberValue);
                        }

                        break;
                    }

                    case OpCode.Sub: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on Sub");
                        stack[sp++] = new NoValue(a.NumberValue - b.NumberValue);
                        break;
                    }

                    case OpCode.Mul: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on Mul");
                        stack[sp++] = new NoValue(a.NumberValue * b.NumberValue);
                        break;
                    }

                    case OpCode.Div: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];
                        double result = b.NumberValue != 0 ? a.NumberValue / b.NumberValue : double.NaN;

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on Div");
                        stack[sp++] = new NoValue(result);
                        break;
                    }

                    case OpCode.Mod: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];
                        double result = b.NumberValue != 0 ? a.NumberValue % b.NumberValue : double.NaN;

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on Mod");
                        stack[sp++] = new NoValue(result);
                        break;
                    }

                    case OpCode.Negate: {
                        NoValue a = stack[--sp];

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on Negate");
                        stack[sp++] = new NoValue(-a.NumberValue);
                        break;
                    }

                    case OpCode.Equal: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on Equal");
                        stack[sp++] = new NoValue(a.Equals(b));
                        break;
                    }

                    case OpCode.NotEqual: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on NotEqual");
                        stack[sp++] = new NoValue(!a.Equals(b));
                        break;
                    }

                    case OpCode.LessThan: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];
                        bool result = (a.Type == ValueType.String && b.Type == ValueType.String)
                            ? string.CompareOrdinal(a.StringValue, b.StringValue) < 0
                            : a.NumberValue < b.NumberValue;

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on LessThan");
                        stack[sp++] = new NoValue(result);
                        break;
                    }

                    case OpCode.LessThanOrEqual: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];
                        bool result = (a.Type == ValueType.String && b.Type == ValueType.String)
                            ? string.CompareOrdinal(a.StringValue, b.StringValue) <= 0
                            : a.NumberValue <= b.NumberValue;

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on LessThanOrEqual");
                        stack[sp++] = new NoValue(result);
                        break;
                    }

                    case OpCode.GreaterThan: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];
                        bool result = (a.Type == ValueType.String && b.Type == ValueType.String)
                            ? string.CompareOrdinal(a.StringValue, b.StringValue) > 0
                            : a.NumberValue > b.NumberValue;

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on GreaterThan");
                        stack[sp++] = new NoValue(result);
                        break;
                    }

                    case OpCode.GreaterThanOrEqual: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];
                        bool result = (a.Type == ValueType.String && b.Type == ValueType.String)
                            ? string.CompareOrdinal(a.StringValue, b.StringValue) >= 0
                            : a.NumberValue >= b.NumberValue;

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on GreaterThanOrEqual");
                        stack[sp++] = new NoValue(result);
                        break;
                    }

                    case OpCode.And: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on And");
                        stack[sp++] = new NoValue(a.IsTruthy() && b.IsTruthy());
                        break;
                    }

                    case OpCode.Or: {
                        NoValue b = stack[--sp];
                        NoValue a = stack[--sp];

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on Or");
                        stack[sp++] = new NoValue(a.IsTruthy() || b.IsTruthy());
                        break;
                    }

                    case OpCode.Not: {
                        NoValue a = stack[--sp];
                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on Not");
                        stack[sp++] = new NoValue(!a.IsTruthy());
                        break;
                    }

                    case OpCode.Jump:
                        ip = inst.Operand;
                        break;

                    case OpCode.JumpIfFalse: {
                        NoValue cond = stack[--sp];
                        if (!cond.IsTruthy()) {
                            ip = inst.Operand;
                        }

                        break;
                    }

                    case OpCode.JumpIfTrue: {
                        NoValue cond = stack[--sp];
                        if (cond.IsTruthy()) {
                            ip = inst.Operand;
                        }

                        break;
                    }

                    case OpCode.ShortCircuitAnd: {
                        NoValue cond = stack[sp - 1];
                        if (!cond.IsTruthy()) {
                            ip = inst.Operand;
                        }

                        break;
                    }

                    case OpCode.ShortCircuitOr: {
                        NoValue cond = stack[sp - 1];
                        if (cond.IsTruthy()) {
                            ip = inst.Operand;
                        }

                        break;
                    }

                    case OpCode.Call: {
                        int funcIdx = inst.Operand;
                        if (funcIdx < 0 || funcIdx >= functions.Length) {
                            ThrowIndexOutOfRange($"Function index {funcIdx} out of bounds.");
                        }

                        CompiledFunction targetFunc = functions[funcIdx];
                        int paramCount = targetFunc.ParamCount;
                        int newLocalBase = nextLocalSlot;
                        nextLocalSlot += targetFunc.LocalCount;

                        if (nextLocalSlot >= MaxLocalsSize) {
                            ThrowStackOverflow($"Locals overflow at slot {nextLocalSlot}.");
                        }
                        if (frameCount >= MaxFramesSize) {
                            ThrowStackOverflow("Call frame depth limit exceeded.");
                        }

                        Array.Clear(locals, newLocalBase, targetFunc.LocalCount);

                        sp -= paramCount;
                        if (paramCount > 0) {
                            Array.Copy(stack, sp, locals, newLocalBase, paramCount);
                        }

                        frames[frameCount++] = new CallFrame {
                            Code = code,
                            IP = ip,
                            StackBase = sp,
                            LocalBase = localBase
                        };

                        localBase = newLocalBase;
                        code = targetFunc.Code;
                        codeLength = code.Length;
                        ip = 0;
                        break;
                    }

                    case OpCode.Return: {
                        NoValue retVal = (sp > 0) ? stack[--sp] : NoValue.Null;

                        if (frameCount == 0) {
                            return;
                        }

                        nextLocalSlot = localBase;

                        frameCount--;
                        CallFrame frame = frames[frameCount];
                        sp = frame.StackBase;
                        localBase = frame.LocalBase;
                        ip = frame.IP;
                        code = frame.Code;
                        codeLength = code.Length;

                        if (sp >= MaxStackSize) ThrowStackOverflow("Stack overflow on Return");
                        stack[sp++] = retVal;
                        break;
                    }

                    case OpCode.StartTimer:
                        timer = Stopwatch.StartNew();
                        break;

                    case OpCode.StopTimer:
                        if (timer != null) {
                            timer.Stop();
                            Console.WriteLine($"[Timer] {timer.Elapsed.TotalMilliseconds:F4} ms");
                            timer = null;
                        }

                        break;

                    case OpCode.Print: {
                        NoValue val = stack[--sp];
                        Console.WriteLine(val.ToString());
                        break;
                    }

                    case OpCode.Pop:
                        sp--;
                        break;
                }
            }
        }
    }

    internal class NoSharpExecutor {
        private const string GlobalsJson =
            "{\"print\":{\"parameters\":[\"any\"],\"returns\":\"void\"},\"startTimer\":{\"parameters\":[],\"returns\":\"void\"},\"stopTimer\":{\"parameters\":[],\"returns\":\"void\"}}";

        [DllImport("nosharp_parser.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr nosharp_parse([MarshalAs(UnmanagedType.LPStr)] string source, [MarshalAs(UnmanagedType.LPStr)] string globals);

        [DllImport("nosharp_parser.dll", CallingConvention = CallingConvention.Cdecl)]
        private static extern void nosharp_free_string(IntPtr ptr);

        private static void Main(string[] args) {
            string filePath = args.Length > 0 ? args[0] : "main.no";

            if (!File.Exists(filePath)) {
                string fallbackPath = Path.Combine(AppContext.BaseDirectory, "main.nos");
                if (File.Exists(fallbackPath)) {
                    filePath = fallbackPath;
                } else {
                    Console.WriteLine($"❌ Error: Source file not found at '{filePath}' or '{fallbackPath}'.");
                    return;
                }
            }

            Console.WriteLine($"Parsing No# code from: {filePath} via nosharp_parser.dll...");
            string sourceCode = File.ReadAllText(filePath);

            IntPtr ptr = nosharp_parse(sourceCode, GlobalsJson);
            if (ptr == IntPtr.Zero) {
                Console.WriteLine("❌ Error: nosharp_parse returned a null pointer.");
                return;
            }

            string jsonResult = Marshal.PtrToStringUTF8(ptr) ?? Marshal.PtrToStringAnsi(ptr) ?? string.Empty;
            nosharp_free_string(ptr);
            Console.WriteLine(jsonResult);

            try {
                using JsonDocument doc = JsonDocument.Parse(jsonResult);
                JsonElement root = doc.RootElement;

                bool success = root.GetProperty("success").GetBoolean();
                if (!success) {
                    JsonElement errors = root.GetProperty("errors");

                    if (errors.GetArrayLength() > 0) {
                        foreach (JsonElement error in errors.EnumerateArray()) {
                            string message = error.GetProperty("message").GetString() ?? "Unknown parse error";
                            if (error.TryGetProperty("rendered", out JsonElement rendered) &&
                                rendered.ValueKind == JsonValueKind.String) {
                                Console.WriteLine(rendered.GetString());
                            } else {
                                Console.WriteLine($"❌ Parse failed: {message}");
                            }
                        }
                    } else {
                        Console.WriteLine("❌ Parse failed: Unknown parse error");
                    }

                    return;
                }

                JsonElement commands = root.GetProperty("commands");

                BytecodeCompiler compiler = new BytecodeCompiler();
                CompiledProgram program = compiler.Compile(commands);

                VM vm = new VM();
                vm.Execute(program);
            } catch (Exception ex) {
                Console.WriteLine($"❌ Execution error:\n{ex}");
            }
        }
    }
}
