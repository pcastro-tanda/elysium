klass = "Foo".safe_constantize
              ^^^^^^^^^^^^^^^^ Sorbet/ConstantsFromStrings: Don't use `safe_constantize`, it makes the code harder to understand, less editor-friendly, and impossible to analyze. Replace `safe_constantize` with a case/when or a hash.
