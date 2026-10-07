klass = const_get("Foo")
        ^^^^^^^^^ Sorbet/ConstantsFromStrings: Don't use `const_get`, it makes the code harder to understand, less editor-friendly, and impossible to analyze. Replace `const_get` with a case/when or a hash.
