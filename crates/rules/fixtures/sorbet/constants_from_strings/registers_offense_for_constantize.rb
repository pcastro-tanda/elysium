klass = "Foo".constantize
              ^^^^^^^^^^^ Sorbet/ConstantsFromStrings: Don't use `constantize`, it makes the code harder to understand, less editor-friendly, and impossible to analyze. Replace `constantize` with a case/when or a hash.
