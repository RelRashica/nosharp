<img align="right" width="220" src="https://github.com/RelRashica/nosharp/raw/main/No%23%20logo.png" alt="No# logo" style="background: transparent; border: none; box-shadow: none;" />

<h1 style="border-bottom: none; margin: 0;">No#</h1>
No# (`/ˈnoʊ.ʃɑːrp/`) is a small, interpreted and early-phase C# inspired programming language.

This language is made for fun and not for serious reasons, although this may not be regularly updated, it could see improvements in the future.
<br clear="right"/>

# Architecture 
- **Frontend (Rust):** Lexer, parser and error handler. (via `chumsky`, `logos` and `ariadne`)
- **Host (C# .NET):** A massive monolithic 1,000-line Native AOT-compiled runner (`NoSharpHost.exe`) that executes scripts with zero runtime dependencies.

# Project Layout
```
nosharp/
├── NoSharp/          # Sample scripts (.nos)
├── NosharpHost/      # C# .NET Native AOT host
├── src/              # Rust core implementation
└── Cargo.toml        # Rust package configuration
```

# How to get started
1. Clone it:
   
   ```
   git clone [https://github.com/RelRashica/nosharp.git](https://github.com/RelRashica/nosharp.git)
   ```
2. Navigate yourself to `NosharpHost`
   
   Run the following (.NET is needed for this):
   ```
   dotnet publish -c Release -r win-x64 -p:PublishAot=true --self-contained true  
   ```
   And then just move the `NosharpHost.exe` from `NosharpHost\bin\Release\net10.0\win-x64\publish` to `NoSharp`, next to `main.nos`

# Known Issues
1. Error Handler points to wrong place **sometimes**
2. No tooling, CLI and whatsoever is currently available
3. Modulos (`%`) is not available yet
4. The Language lacks allot of features, as it is very new
5. The Language is kinda slow on some things (like string concat for example)

# License
No# implementation is distributed under the terms of MIT License.

When No# is integrated into external projects, we ask that you honor the license agreement and include No# attribution into the user-facing product documentation. Attribution making use of the No# logo is also encouraged when reasonable.
